#!/usr/bin/env python3
"""Derive crates/tellegen-synth/data/subnational_fuel_mix.json, the nameplate
capacity mix of each US state, in the generator's fuels.

usage: python3 -I scripts/derive-subnational-fuel-mix.py eia8602025.zip > \
           crates/tellegen-synth/data/subnational_fuel_mix.json

eia860YYYY.zip is the final Form EIA-860 detailed data for the year
(https://www.eia.gov/electricity/data/eia860/), a US government work in the
public domain. Only its member 3_1_Generator_YYYY.xlsx, sheet "Operable", is
read: one row per utility-scale generator (plants of 1 MW or more), with its
state, prime mover, status, nameplate capacity and energy sources. The
archive is third-party input: the xlsx is opened with zipfile and xml.etree
(sharedStrings and sheet XML only), every number is validated, and nothing in
it is executed.

Generators with status OP (operating) are counted; standby (SB) and
out-of-service (OS, OA) units are not. Each state's entry is its total
nameplate MW and the share of it in each of the generator's fuels, rounded to
6 decimals by largest remainder so that the shares sum to exactly 1.
"""
import argparse
import hashlib
import io
import json
import re
import sys
import zipfile
from fractions import Fraction
from pathlib import Path
from xml.etree import ElementTree

# The generator's fuels, in its order (tellegen_synth::request::Fuel).
FUELS = ('gas', 'coal', 'nuclear', 'hydro', 'wind', 'solar', 'storage', 'other')
USPS = {
    'AK', 'AL', 'AR', 'AZ', 'CA', 'CO', 'CT', 'DC', 'DE', 'FL', 'GA', 'HI', 'IA', 'ID', 'IL', 'IN', 'KS',
    'KY', 'LA', 'MA', 'MD', 'ME', 'MI', 'MN', 'MO', 'MS', 'MT', 'NC', 'ND', 'NE', 'NH', 'NJ', 'NM', 'NV',
    'NY', 'OH', 'OK', 'OR', 'PA', 'RI', 'SC', 'SD', 'TN', 'TX', 'UT', 'VA', 'VT', 'WA', 'WI', 'WV', 'WY',
}
# Prime movers that store energy (EIA-860 layout, Table 2): BA battery, CE
# compressed air, FW flywheel, PS reversible hydraulic turbine (pumped
# storage), ES other storage. A storage prime mover decides the fuel whatever
# the energy source: pumped storage reports WAT, compressed air reports NG.
# CP, the storage of a concentrating solar plant, is not here: its energy
# source is SUN, and it counts as solar thermal.
STORAGE_PRIME_MOVERS = {'BA', 'CE', 'FW', 'PS', 'ES'}
# WAT at a prime mover other than PS: HY is a conventional hydroelectric
# turbine; HA, HB and HK are hydrokinetic (axial flow, wave buoy, other).
CONVENTIONAL_HYDRO = 'HY'
# Energy Source 1 code (EIA-860 layout, Table 28 "Energy Source Codes and Heat
# Content") -> fuel. Every code in Table 28 is listed, so a code EIA adds later
# stops the script instead of falling into a fuel unreviewed.
FUEL_OF_SOURCE = {
    # Coal, EIA's coal group: anthracite, bituminous, lignite, coal-derived
    # synthesis gas, subbituminous, waste/other coal, refined coal.
    'ANT': 'coal', 'BIT': 'coal', 'LIG': 'coal', 'SGC': 'coal', 'SUB': 'coal', 'WC': 'coal', 'RC': 'coal',
    # Natural gas and other gases, EIA's group of that name: blast furnace
    # gas, natural gas, hydrogen, other gas.
    'BFG': 'gas', 'NG': 'gas', 'H2': 'gas', 'OG': 'gas',
    # Petroleum products: distillate, jet fuel, kerosene, petroleum coke,
    # gaseous propane, residual fuel oil, synthesis gas from petroleum coke,
    # waste/other oil.
    'DFO': 'other', 'JF': 'other', 'KER': 'other', 'PC': 'other', 'PG': 'other', 'RFO': 'other',
    'SGP': 'other', 'WO': 'other',
    # Solid, liquid and gaseous biomass: agricultural by-products, municipal
    # solid waste, other biomass solids, wood/wood waste solids, other biomass
    # liquids, sludge waste, black liquor, wood waste liquids, landfill gas,
    # other biomass gas.
    'AB': 'other', 'MSW': 'other', 'OBS': 'other', 'WDS': 'other', 'OBL': 'other', 'SLW': 'other',
    'BLQ': 'other', 'WDL': 'other', 'LFG': 'other', 'OBG': 'other',
    # Other renewables.
    'SUN': 'solar', 'WND': 'wind', 'GEO': 'other',
    'WAT': 'hydro',  # conventional hydro only; see CONVENTIONAL_HYDRO
    # All other: nuclear, purchased steam, waste heat, tire-derived fuel,
    # electricity for storage, other.
    'NUC': 'nuclear', 'PUR': 'other', 'WH': 'other', 'TDF': 'other', 'MWH': 'storage', 'OTH': 'other',
}
SOURCE_URL = 'https://www.eia.gov/electricity/data/eia860/xls/eia860{year}.zip'
# Ceilings on what is decompressed from the archive.
MAX_XLSX_BYTES = 64 << 20
MAX_PART_BYTES = 512 << 20
SPREADSHEET = '{http://schemas.openxmlformats.org/spreadsheetml/2006/main}'
RELATIONSHIPS = '{http://schemas.openxmlformats.org/officeDocument/2006/relationships}'


# --- xlsx, read with zipfile and xml.etree only (as in derive-subnational-demand.py)

def zip_member(archive, name, limit):
    info = archive.getinfo(name)
    if info.file_size > limit:
        sys.exit(f'{name}: {info.file_size} bytes uncompressed, over the {limit} byte limit')
    return archive.open(info)


def xml_part(book, name):
    # SpreadsheetML parts never declare a document type; refusing one keeps
    # entity expansion out of the parse. A declaration precedes the root
    # element, so the prologue is enough to check.
    with zip_member(book, name, MAX_PART_BYTES) as handle:
        head = handle.read(1 << 16)
    if b'<!DOCTYPE' in head or b'<!ENTITY' in head:
        sys.exit(f'{name}: document type declarations are not accepted')
    return zip_member(book, name, MAX_PART_BYTES)


def column(ref):
    letters = re.match(r'[A-Z]+', ref)
    if letters is None:
        raise ValueError(f'bad cell reference {ref!r}')
    index = 0
    for ch in letters.group(0):
        index = index * 26 + ord(ch) - 64
    return index - 1


def text_of(node):
    # A shared or inline string is plain <t> or rich-text runs <r><t>;
    # phonetic hints (<rPh>) are not part of the value.
    parts = []
    for child in node:
        if child.tag == SPREADSHEET + 't':
            parts.append(child.text or '')
        elif child.tag == SPREADSHEET + 'r':
            parts.extend(t.text or '' for t in child.findall(SPREADSHEET + 't'))
    return ''.join(parts)


def xlsx_rows(xlsx_bytes, sheet):
    """Yield each row of `sheet` as a list of cell strings (None when empty)."""
    book = zipfile.ZipFile(io.BytesIO(xlsx_bytes))
    with xml_part(book, 'xl/workbook.xml') as handle:
        workbook = ElementTree.parse(handle).getroot()
    with xml_part(book, 'xl/_rels/workbook.xml.rels') as handle:
        rels = {r.get('Id'): r.get('Target') for r in ElementTree.parse(handle).getroot()}
    target = None
    for node in workbook.iter(SPREADSHEET + 'sheet'):
        if node.get('name') == sheet:
            target = rels.get(node.get(RELATIONSHIPS + 'id'))
    if target is None:
        sys.exit(f'no sheet {sheet!r}')
    target = target.lstrip('/')
    path = target if target.startswith('xl/') else 'xl/' + target
    strings = []
    if 'xl/sharedStrings.xml' in book.namelist():
        with xml_part(book, 'xl/sharedStrings.xml') as handle:
            for _, node in ElementTree.iterparse(handle):
                if node.tag == SPREADSHEET + 'si':
                    strings.append(text_of(node))
                    node.clear()
    with xml_part(book, path) as handle:
        for _, node in ElementTree.iterparse(handle):
            if node.tag != SPREADSHEET + 'row':
                continue
            cells = {}
            for cell in node.findall(SPREADSHEET + 'c'):
                kind = cell.get('t')
                value = cell.find(SPREADSHEET + 'v')
                if kind == 's':
                    try:
                        text = strings[int(value.text)]
                    except (AttributeError, IndexError, TypeError, ValueError):
                        sys.exit(f'{path}: bad shared string reference')
                elif kind == 'inlineStr':
                    inline = cell.find(SPREADSHEET + 'is')
                    text = text_of(inline) if inline is not None else ''
                else:
                    text = value.text if value is not None else None
                ref = cell.get('r')
                cells[column(ref) if ref else max(cells, default=-1) + 1] = text
            node.clear()
            width = max(cells) + 1 if cells else 0
            yield [cells.get(i) for i in range(width)]


# --- method ---------------------------------------------------------------------

def fuel_of(prime_mover, source):
    if prime_mover in STORAGE_PRIME_MOVERS:
        return 'storage'
    if source not in FUEL_OF_SOURCE:
        sys.exit(f'energy source {source!r} is not in FUEL_OF_SOURCE; review the mapping')
    if source == 'WAT' and prime_mover != CONVENTIONAL_HYDRO:
        return 'other'
    return FUEL_OF_SOURCE[source]


def megawatts(text):
    try:
        value = Fraction((text or '').strip())
    except (ValueError, ZeroDivisionError):
        return None
    return value if value >= 0 else None


def shares(capacity):
    """Shares in millionths, largest remainder, ties in FUELS order."""
    total = sum(capacity.values())
    exact = {fuel: capacity[fuel] * 10**6 / total for fuel in FUELS}
    floor = {fuel: int(exact[fuel]) for fuel in FUELS}
    spare = 10**6 - sum(floor.values())
    for fuel in sorted(FUELS, key=lambda f: (-(exact[f] - floor[f]), FUELS.index(f)))[:spare]:
        floor[fuel] += 1
    return floor


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('archive', type=Path, help='eia860YYYY.zip (EIA-860, final)')
    parser.add_argument('--year', type=int, default=2025)
    args = parser.parse_args()
    year = args.year
    raw = args.archive.read_bytes()
    member = f'3_1_Generator_Y{year}.xlsx'
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        if member not in archive.namelist():
            sys.exit(f'{args.archive}: no {member}')
        with zip_member(archive, member, MAX_XLSX_BYTES) as handle:
            xlsx_bytes = handle.read()
    rows = xlsx_rows(xlsx_bytes, 'Operable')
    title = (next(rows) or [''])[0] or ''
    if not title.startswith(f'{year} Form EIA-860 Data - Schedule 3'):
        sys.exit(f'{member}: unexpected title {title!r}')
    header = next(rows)
    wanted = ('State', 'Prime Mover', 'Nameplate Capacity (MW)', 'Status', 'Energy Source 1', 'Technology')
    if any(name not in header for name in wanted):
        sys.exit(f'{member}: missing one of the columns {wanted}')
    index = {name: header.index(name) for name in wanted}
    capacity = {state: dict.fromkeys(FUELS, Fraction(0)) for state in sorted(USPS)}
    by_technology = {}
    generators = 0
    for row in rows:
        row = row + [None] * (len(header) - len(row))
        state = row[index['State']]
        if state is None and (row[0] or '').startswith('NOTE:'):
            continue
        if state not in USPS:
            sys.exit(f'{member}: unexpected state {state!r} in {row[:7]}')
        if row[index['Status']] != 'OP':
            continue
        mw = megawatts(row[index['Nameplate Capacity (MW)']])
        if mw is None:
            sys.exit(f'{member}: bad nameplate capacity in {row[:7]}')
        prime_mover = (row[index['Prime Mover']] or '').strip()
        source = (row[index['Energy Source 1']] or '').strip()
        fuel = fuel_of(prime_mover, source)
        capacity[state][fuel] += mw
        key = (fuel, row[index['Technology']] or '')
        by_technology[key] = by_technology.get(key, Fraction(0)) + mw
        generators += 1
    for (fuel, technology), mw in sorted(by_technology.items()):
        print(f'{fuel:8} {float(mw):12.1f} MW  {technology}', file=sys.stderr)
    for state, mix in capacity.items():
        if sum(mix.values()) <= 0:
            sys.exit(f'{member}: no operating capacity in {state}')
    total_us = sum(sum(mix.values()) for mix in capacity.values())

    lines = [
        '{',
        '  "schema": "tellegen.synth-subnational-fuel-mix/1",',
        f'  "year": {year},',
        '  "country": "USA",',
        '  "sources": [',
        '    ' + json.dumps({
            'name': f'EIA-860 {year} final, 3_1_Generator_Y{year}.xlsx, sheet Operable',
            'url': SOURCE_URL.format(year=year), 'file': args.archive.name,
            'sha256': hashlib.sha256(raw).hexdigest()}, sort_keys=True),
        '  ],',
        '  "licence": "Public domain: EIA data are works of the US government (17 U.S.C. 105)",',
        f'  "note": "Nameplate capacity of operating (status OP) utility-scale generators by state, {generators} '
        f'generators and {float(total_us) / 1e3:.1f} GW in all, derived by scripts/derive-subnational-fuel-mix.py. '
        f'Entries are the total MW and each fuel\'s share of it, rounded to 6 decimals by largest remainder so they '
        f'sum to 1. Small-scale generation (plants under 1 MW, most rooftop solar) is not in EIA-860.",',
        '  "method": ' + json.dumps(
            'Fuel by prime mover, then Energy Source 1. Storage: prime movers BA (batteries), CE (compressed air), '
            'FW (flywheels), PS (pumped storage) and ES (other storage). Hydro: WAT at an HY conventional '
            'hydroelectric turbine; hydrokinetic WAT (HA, HB, HK) is other. Gas: NG, BFG, OG, H2 (EIA\'s natural '
            'gas and other gases). Coal: ANT, BIT, LIG, SUB, WC, RC, SGC (EIA\'s coal group). Nuclear: NUC. Wind: '
            'WND, onshore and offshore. Solar: SUN, photovoltaic and solar thermal. Other: petroleum products, '
            'geothermal, biomass, wood and waste, landfill gas, municipal solid waste, purchased steam, waste heat, '
            'tire-derived fuel and anything else.') + ',',
        '  "fuels": ' + json.dumps(list(FUELS)) + ',',
        '  "subdivisions": {',
    ]
    codes = sorted(USPS)
    for i, state in enumerate(codes):
        comma = ',' if i + 1 < len(codes) else ''
        mix = capacity[state]
        parts = shares(mix)
        text = ', '.join(f'"{fuel}": {parts[fuel] // 10**6}.{parts[fuel] % 10**6:06d}' for fuel in FUELS)
        lines.append(f'    "US-{state}": {{"total_mw": {float(sum(mix.values())):.1f}, "shares": {{{text}}}}}{comma}')
    lines += ['  }', '}']
    print('\n'.join(lines))


if __name__ == '__main__':
    main()
