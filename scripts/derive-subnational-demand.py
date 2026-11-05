#!/usr/bin/env python3
"""Derive crates/tellegen-synth/data/subnational_demand.json, the US state
electricity demand table behind the generator's sub-national demand basis.

usage: python3 -I scripts/derive-subnational-demand.py \
           --sales f8612025.zip --population NST-EST2025-ALLDATA.csv \
           --hourly EIA930_BALANCE_2025_Jan_Jun.csv EIA930_BALANCE_2025_Jul_Dec.csv \
           > crates/tellegen-synth/data/subnational_demand.json

The inputs are US government works in the public domain:

- f861YYYY.zip, the final Form EIA-861 detailed data for the year
  (https://www.eia.gov/electricity/data/eia861/). Only its member
  Sales_Ult_Cust_YYYY.xlsx, sheet "States", is read: retail sales to ultimate
  customers by utility, state, part and balancing authority (BA Code).
- NST-ESTYYYY-ALLDATA.csv, the Census Bureau's Vintage YYYY state population
  estimates (https://www2.census.gov/programs-surveys/popest/datasets/).
  POPESTIMATEYYYY, the July 1 estimate, is read for SUMLEV 040.
- EIA930_BALANCE_YYYY_Jan_Jun.csv and _Jul_Dec.csv, the EIA-930 six-month
  hourly balancing-authority files
  (https://www.eia.gov/electricity/gridmonitor/sixMonthFiles/). Only
  "Demand (MW) (Adjusted)" is read, keyed by "UTC Time at End of Hour".

Every input is third-party data: the xlsx is opened with zipfile and
xml.etree (sharedStrings and sheet XML only), the CSVs with csv, every number
is validated, and nothing in them is executed.

Entries are [year, TWh, population, peak_factor or null], keyed by ISO 3166-2
code. A state's peak factor is the annual maximum over the mean of the hourly
sum of the BAs that sell at least --ba-share of their retail sales inside it,
provided those BAs carry at least --coverage of the state's sales; otherwise
it is null and the generator uses its default.
"""
import argparse
import csv
import hashlib
import io
import json
import math
import re
import sys
import zipfile
from datetime import datetime, timezone
from pathlib import Path
from xml.etree import ElementTree

# FIPS state code -> (USPS code, Census name). ISO 3166-2:US subdivision codes
# are "US-" plus the USPS code; DC is US-DC.
STATES = {
    '01': ('AL', 'Alabama'), '02': ('AK', 'Alaska'), '04': ('AZ', 'Arizona'),
    '05': ('AR', 'Arkansas'), '06': ('CA', 'California'), '08': ('CO', 'Colorado'),
    '09': ('CT', 'Connecticut'), '10': ('DE', 'Delaware'),
    '11': ('DC', 'District of Columbia'), '12': ('FL', 'Florida'),
    '13': ('GA', 'Georgia'), '15': ('HI', 'Hawaii'), '16': ('ID', 'Idaho'),
    '17': ('IL', 'Illinois'), '18': ('IN', 'Indiana'), '19': ('IA', 'Iowa'),
    '20': ('KS', 'Kansas'), '21': ('KY', 'Kentucky'), '22': ('LA', 'Louisiana'),
    '23': ('ME', 'Maine'), '24': ('MD', 'Maryland'), '25': ('MA', 'Massachusetts'),
    '26': ('MI', 'Michigan'), '27': ('MN', 'Minnesota'), '28': ('MS', 'Mississippi'),
    '29': ('MO', 'Missouri'), '30': ('MT', 'Montana'), '31': ('NE', 'Nebraska'),
    '32': ('NV', 'Nevada'), '33': ('NH', 'New Hampshire'), '34': ('NJ', 'New Jersey'),
    '35': ('NM', 'New Mexico'), '36': ('NY', 'New York'),
    '37': ('NC', 'North Carolina'), '38': ('ND', 'North Dakota'), '39': ('OH', 'Ohio'),
    '40': ('OK', 'Oklahoma'), '41': ('OR', 'Oregon'), '42': ('PA', 'Pennsylvania'),
    '44': ('RI', 'Rhode Island'), '45': ('SC', 'South Carolina'),
    '46': ('SD', 'South Dakota'), '47': ('TN', 'Tennessee'), '48': ('TX', 'Texas'),
    '49': ('UT', 'Utah'), '50': ('VT', 'Vermont'), '51': ('VA', 'Virginia'),
    '53': ('WA', 'Washington'), '54': ('WV', 'West Virginia'),
    '55': ('WI', 'Wisconsin'), '56': ('WY', 'Wyoming'),
}
USPS = {code for code, _ in STATES.values()}
# Parts of the Sales_Ult_Cust file that carry sales: A bundled, B energy-only,
# D bundled service by restructured retail providers. C, delivery-only, repeats
# the energy of B and is left out, as the file's own footnote prescribes for
# state and US totals.
SALES_PARTS = {'A', 'B', 'D'}
# BA Code values that name no balancing authority.
NO_BA = {'', 'NA'}
BA_CODE = re.compile(r'[A-Z0-9]{2,8}')
SALES_URL = 'https://www.eia.gov/electricity/data/eia861/zip/f861{year}.zip'
POPULATION_URL = ('https://www2.census.gov/programs-surveys/popest/datasets/'
                  '2020-{year}/state/totals/NST-EST{year}-ALLDATA.csv')
HOURLY_URL = 'https://www.eia.gov/electricity/gridmonitor/sixMonthFiles/{name}'
HOURLY_NAMES = ('EIA930_BALANCE_{year}_Jan_Jun.csv', 'EIA930_BALANCE_{year}_Jul_Dec.csv')
# Ceilings on what is decompressed from the archives.
MAX_XLSX_BYTES = 64 << 20
MAX_PART_BYTES = 512 << 20
SPREADSHEET = '{http://schemas.openxmlformats.org/spreadsheetml/2006/main}'
RELATIONSHIPS = '{http://schemas.openxmlformats.org/officeDocument/2006/relationships}'


# --- xlsx, read with zipfile and xml.etree only --------------------------------

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


# --- inputs ---------------------------------------------------------------------

def finite(text):
    try:
        value = float(text.replace(',', '').strip())
    except (AttributeError, ValueError):
        return None
    return value if math.isfinite(value) else None


def read_sales(path, year):
    """State totals and (state, BA) totals of retail sales, MWh."""
    member = f'Sales_Ult_Cust_{year}.xlsx'
    with zipfile.ZipFile(path) as archive:
        if member not in archive.namelist():
            sys.exit(f'{path}: no {member}')
        with zip_member(archive, member, MAX_XLSX_BYTES) as handle:
            xlsx_bytes = handle.read()
    rows = xlsx_rows(xlsx_bytes, 'States')
    groups, measures, fields = next(rows), next(rows), next(rows)
    group = ''
    total_sales = None
    for i, field in enumerate(fields):
        group = (groups[i] if i < len(groups) and groups[i] else group).strip()
        measure = measures[i] if i < len(measures) else None
        if group == 'TOTAL' and measure == 'Sales' and field == 'Megawatthours':
            total_sales = i
    wanted = ('Data Year', 'Part', 'State', 'Ownership', 'BA Code')
    index = {name: fields.index(name) for name in wanted if name in fields}
    if total_sales is None or len(index) != len(wanted):
        sys.exit(f'{member}: unexpected header {fields}')
    states = dict.fromkeys(sorted(USPS), 0.0)
    by_ba = {}
    behind_meter = 0.0
    footnote = ''
    for row in rows:
        row = row + [None] * (len(fields) - len(row))
        if row[index['Data Year']] != str(year):
            footnote += row[0] or ''
            continue
        part, state, ba = row[index['Part']], row[index['State']], (row[index['BA Code']] or '').strip()
        if part not in SALES_PARTS | {'C'} or state not in USPS or not (ba in NO_BA or BA_CODE.fullmatch(ba)):
            sys.exit(f'{member}: unexpected row {row[:9]}')
        text = (row[total_sales] or '').strip()
        mwh = 0.0 if text in ('', '.') else finite(text)
        if mwh is None:
            sys.exit(f'{member}: bad sales value {text!r} in {row[:9]}')
        if part not in SALES_PARTS:
            continue
        states[state] += mwh
        by_ba[(state, ba)] = by_ba.get((state, ba), 0.0) + mwh
        if row[index['Ownership']] == 'Behind the Meter':
            behind_meter += mwh
    if 'only Parts (A,B & D) for Sales' not in footnote:
        sys.exit(f'{member}: the footnote on summing parts has changed; review SALES_PARTS')
    for state, mwh in states.items():
        if mwh <= 0:
            sys.exit(f'{member}: no sales for {state}')
    return states, by_ba, behind_meter


def read_population(path, year):
    raw = path.read_bytes()
    rows = csv.DictReader(io.StringIO(raw.decode('utf-8-sig'), newline=''))
    column_name = f'POPESTIMATE{year}'
    population = {}
    for row in rows:
        if row.get('SUMLEV') != '040':
            continue
        fips = row.get('STATE') or ''
        if fips not in STATES:
            continue
        code, name = STATES[fips]
        if row.get('NAME') != name:
            sys.exit(f'{path.name}: FIPS {fips} is {row.get("NAME")!r}, expected {name!r}')
        value = finite(row.get(column_name))
        if value is None or value <= 0 or value != int(value):
            sys.exit(f'{path.name}: bad {column_name} for {name}')
        population[code] = int(value)
    if set(population) != USPS:
        sys.exit(f'{path.name}: no {column_name} for {sorted(USPS - set(population))}')
    return raw, population


def read_hourly(paths, year):
    """Adjusted hourly demand by BA: {ba: {utc_hour_index: MW}}."""
    series = {}
    raws = []
    for path in paths:
        raw = path.read_bytes()
        raws.append(raw)
        rows = csv.DictReader(io.StringIO(raw.decode('utf-8-sig'), newline=''))
        needed = ('Balancing Authority', 'Data Date', 'UTC Time at End of Hour', 'Demand (MW) (Adjusted)')
        if rows.fieldnames is None or any(name not in rows.fieldnames for name in needed):
            sys.exit(f'{path.name}: missing one of the columns {needed}')
        for row in rows:
            ba = (row['Balancing Authority'] or '').strip()
            if not BA_CODE.fullmatch(ba):
                sys.exit(f'{path.name}: bad balancing authority {ba!r}')
            if not (row['Data Date'] or '').endswith(f'/{year}'):
                sys.exit(f'{path.name}: {ba} row dated {row["Data Date"]!r}, outside {year}')
            try:
                stamp = datetime.strptime(row['UTC Time at End of Hour'].strip(), '%m/%d/%Y %I:%M:%S %p')
            except (AttributeError, ValueError):
                sys.exit(f'{path.name}: bad UTC time {row["UTC Time at End of Hour"]!r}')
            if stamp.minute or stamp.second:
                sys.exit(f'{path.name}: {ba} time {stamp} is not on the hour')
            hour = int(stamp.replace(tzinfo=timezone.utc).timestamp()) // 3600
            mw = finite(row['Demand (MW) (Adjusted)'] or '')
            if mw is None:
                continue
            known = series.setdefault(ba, {})
            if hour in known and known[hour] != mw:
                sys.exit(f'{path.name}: {ba} reports two demands for {stamp} UTC')
            known[hour] = mw
    return raws, series


# --- method ---------------------------------------------------------------------

def plausible(values):
    """Keep positive hours no larger than twice the nearest-rank 99.9th
    percentile of the BA's positive hours."""
    positive = {hour: mw for hour, mw in values.items() if mw > 0}
    if not positive:
        return {}
    ranked = sorted(positive.values())
    p999 = ranked[max(math.ceil(0.999 * len(ranked)) - 1, 0)]
    return {hour: mw for hour, mw in positive.items() if mw <= 2.0 * p999}


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('--sales', type=Path, required=True, help='f861YYYY.zip (EIA-861, final)')
    parser.add_argument('--population', type=Path, required=True, help='NST-ESTYYYY-ALLDATA.csv (Census)')
    parser.add_argument('--hourly', type=Path, nargs=2, required=True, help='the two EIA930_BALANCE_YYYY CSVs')
    parser.add_argument('--year', type=int, default=2025)
    parser.add_argument('--ba-share', type=float, default=0.8,
                        help='share of a BA\'s retail sales that must lie in the state')
    parser.add_argument('--coverage', type=float, default=0.8,
                        help='share of the state\'s sales its in-state BAs must carry')
    parser.add_argument('--min-hours', type=float, default=0.95,
                        help='share of the year\'s hours a BA must report to be used')
    args = parser.parse_args()
    year = args.year
    hourly_names = [name.format(year=year) for name in HOURLY_NAMES]
    if sorted(p.name for p in args.hourly) != sorted(hourly_names):
        sys.exit(f'--hourly expects {hourly_names}')
    hourly_paths = sorted(args.hourly, key=lambda p: hourly_names.index(p.name))

    sales, by_ba, behind_meter = read_sales(args.sales, year)
    population_raw, population = read_population(args.population, year)
    hourly_raws, hourly = read_hourly(hourly_paths, year)
    hours_in_year = (datetime(year + 1, 1, 1) - datetime(year, 1, 1)).days * 24

    ba_total = {}
    for (state, ba), mwh in by_ba.items():
        if ba not in NO_BA:
            ba_total[ba] = ba_total.get(ba, 0.0) + mwh
    clean = {ba: plausible(values) for ba, values in hourly.items()}
    usable = {ba for ba, values in clean.items() if len(values) >= args.min_hours * hours_in_year}

    factors = {}
    basis = {}
    for state in sorted(USPS):
        inside = sorted(
            ba for (s, ba), mwh in by_ba.items()
            if s == state and ba not in NO_BA and ba_total[ba] > 0 and mwh / ba_total[ba] >= args.ba_share
        )
        bas = [ba for ba in inside if ba in usable]
        coverage = sum(by_ba[(state, ba)] for ba in bas) / sales[state]
        missing = [ba for ba in inside if ba not in usable]
        print(f'{state}: in-state BAs {inside or "none"}'
              f'{f" (no hourly data: {missing})" if missing else ""}, coverage {coverage:.3f}',
              file=sys.stderr)
        if not bas or coverage < args.coverage:
            factors[state] = None
            continue
        common = sorted(set.intersection(*(set(clean[ba]) for ba in bas)))
        totals = [sum(clean[ba][hour] for ba in bas) for hour in common]
        mean = math.fsum(totals) / len(totals)
        peak = max(totals)
        peak_hour = datetime.fromtimestamp(common[totals.index(peak)] * 3600, tz=timezone.utc)
        factors[state] = round(peak / mean, 3)
        basis[state] = {'bas': bas, 'coverage': round(coverage, 3), 'hours': len(totals),
                        'mean_mw': round(mean), 'peak_mw': round(peak),
                        'peak_hour_ending_utc': peak_hour.strftime('%Y-%m-%dT%H:00Z')}

    dropped = {ba: len(hourly[ba]) - len(clean[ba]) for ba in sorted({b for v in basis.values() for b in v['bas']})}
    dropped_text = ', '.join(f'{ba} {n}' for ba, n in dropped.items() if n) or 'none'
    with_factor = sum(1 for f in factors.values() if f is not None)
    sources = [
        {'name': f'EIA-861 {year} final, Sales_Ult_Cust_{year}.xlsx, sheet States',
         'url': SALES_URL.format(year=year), 'file': args.sales.name,
         'sha256': hashlib.sha256(args.sales.read_bytes()).hexdigest()},
        {'name': f'US Census Bureau, Vintage {year} state population estimates, POPESTIMATE{year}',
         'url': POPULATION_URL.format(year=year), 'file': args.population.name,
         'sha256': hashlib.sha256(population_raw).hexdigest()},
    ] + [
        {'name': f'EIA-930 hourly balancing authority data, {path.name}, Demand (MW) (Adjusted)',
         'url': HOURLY_URL.format(name=path.name), 'file': path.name,
         'sha256': hashlib.sha256(raw).hexdigest()}
        for path, raw in zip(hourly_paths, hourly_raws)
    ]
    method = {
        'twh': f'Annual retail sales to ultimate customers, all sectors (TOTAL Sales, MWh), summed over '
               f'Parts A, B and D as the file\'s footnote prescribes for state totals; this includes the '
               f'"Adjustment {year}" rows that stand for short-form (EIA-861S) respondents. Retail sales exclude '
               f'direct use: electricity generated and used on site (industrial and commercial self-generation, '
               f'customer-owned rooftop solar) is not in them, and neither are transmission and distribution '
               f'losses. Third-party-owned behind-the-meter solar sold to the host customer is included '
               f'({behind_meter / 1e6:.3f} TWh nationally).',
        'population': f'Census Bureau Vintage {year} estimate of the resident population on 1 July {year}.',
        'peak_factor': f'Annual peak hourly demand over mean hourly demand, from EIA-930 Demand (MW) (Adjusted) '
                       f'(EIA\'s series with anomalous and missing reports replaced by imputed values). A BA is '
                       f'in a state when at least {args.ba_share:.0%} of its EIA-861 retail sales (by BA Code) lie '
                       f'in that state. When a state\'s in-state BAs that report at least {args.min_hours:.0%} of '
                       f'the year\'s hours carry at least {args.coverage:.0%} of its sales, the factor is the '
                       f'maximum of their hourly sum over its mean, taken over the UTC hours every one of them '
                       f'reports; otherwise it is '
                       f'null and the generator uses its default. Hours that are not physically plausible are '
                       f'dropped per BA before summing: non-positive values and values above twice the BA\'s '
                       f'nearest-rank 99.9th percentile (hours dropped in the BAs used: {dropped_text}). The factor '
                       f'is a ratio of BA demand, which includes losses and is net of behind-the-meter generation, '
                       f'applied to retail sales.',
    }
    lines = [
        '{',
        '  "schema": "tellegen.synth-subnational-demand/1",',
        f'  "year": {year},',
        '  "country": "USA",',
        '  "sources": [',
        ',\n'.join('    ' + json.dumps(source, sort_keys=True) for source in sources),
        '  ],',
        '  "licence": "Public domain: EIA and US Census Bureau data are works of the US government (17 U.S.C. 105)",',
        f'  "note": "Entries are [year, TWh, population, peak_factor] for the 50 states and DC, keyed by ISO 3166-2 '
        f'code, derived by scripts/derive-subnational-demand.py. {with_factor} of the {len(factors)} have a '
        f'peak factor; peak_basis gives, for each, its BAs, their share of the state\'s retail sales, the hours '
        f'used, and the mean, peak and peak hour of their hourly sum.",',
        '  "method": {',
        ',\n'.join(f'    "{key}": {json.dumps(text)}' for key, text in method.items()),
        '  },',
        '  "subdivisions": {',
    ]
    codes = sorted(USPS)
    for i, state in enumerate(codes):
        comma = ',' if i + 1 < len(codes) else ''
        factor = 'null' if factors[state] is None else f'{factors[state]:.3f}'
        lines.append(f'    "US-{state}": [{year}, {sales[state] / 1e6:.3f}, {population[state]}, {factor}]{comma}')
    lines += ['  },', '  "peak_basis": {']
    entries = sorted(basis.items())
    for i, (state, entry) in enumerate(entries):
        comma = ',' if i + 1 < len(entries) else ''
        lines.append(f'    "US-{state}": {json.dumps(entry, sort_keys=True)}{comma}')
    lines += ['  }', '}']
    print('\n'.join(lines))


if __name__ == '__main__':
    main()
