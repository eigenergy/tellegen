#!/usr/bin/env python3
"""Derive crates/tellegen-synth/data/national_demand.json, the national
electricity demand table behind the generator's population demand basis.

usage: python3 -I scripts/derive-national-demand.py OWID_ENERGY_CSV > \
           crates/tellegen-synth/data/national_demand.json

OWID_ENERGY_CSV is Our World in Data's owid-energy-data.csv
(https://github.com/owid/energy-data). Its `electricity_demand` column comes
from Ember's Yearly Electricity Data (CC BY 4.0) and its `population` column
from OWID's population series, compiled from the sources listed at
https://ourworldindata.org/population-sources under their terms. The CSV is
third-party input: it is
read as text, every number is validated, and nothing in it is executed.

Each country keeps its latest year up to --year that states both a positive
demand and a positive population, provided that year is no older than
--oldest; older or missing countries are left out and fall back to the
world figure at run time.
"""
import argparse
import csv
import hashlib
import io
import json
import math
import re
import sys
from pathlib import Path

# Natural Earth codes in the places table that name an OWID country under a
# different code: Palestine, Western Sahara, and the Åland Islands (part of
# Finland's statistics).
ALIASES = {'PSX': 'PSE', 'SAH': 'ESH', 'ALD': 'FIN'}
ISO3 = re.compile(r'[A-Z]{3}')


def number(text):
    try:
        value = float(text)
    except (TypeError, ValueError):
        return None
    return value if math.isfinite(value) and value > 0 else None


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('csv', type=Path)
    parser.add_argument('--year', type=int, default=2024)
    parser.add_argument('--oldest', type=int, default=2019)
    args = parser.parse_args()
    raw = args.csv.read_bytes()
    rows = csv.DictReader(io.StringIO(raw.decode('utf-8'), newline=''))
    latest = {}
    world = None
    for row in rows:
        try:
            year = int(row['year'])
        except (KeyError, TypeError, ValueError):
            continue
        if year > args.year:
            continue
        twh = number(row.get('electricity_demand'))
        population = number(row.get('population'))
        if twh is None or population is None:
            continue
        code = row.get('iso_code') or ''
        if row.get('country') == 'World' and not code and year == args.year:
            world = (twh, population)
        if not ISO3.fullmatch(code) or year < args.oldest:
            continue
        if code not in latest or year > latest[code][0]:
            latest[code] = (year, twh, population)
    if world is None:
        sys.exit(f'no World row for {args.year}')
    aliases = {code: target for code, target in ALIASES.items() if target in latest}
    for code in sorted(set(ALIASES) - set(aliases)):
        print(f'note: {code} -> {ALIASES[code]} dropped: no entry since {args.oldest}', file=sys.stderr)
    years = [entry[0] for entry in latest.values()]
    lines = [
        '{',
        '  "schema": "tellegen.synth-national-demand/1",',
        f'  "year": {args.year},',
        '  "source": "Ember, Yearly Electricity Data, via Our World in Data energy-data '
        '(electricity_demand, population)",',
        '  "source_url": "https://github.com/owid/energy-data",',
        f'  "source_sha256": "{hashlib.sha256(raw).hexdigest()}",',
        '  "licence": "CC BY 4.0",',
        f'  "note": "{years.count(args.year)} of {len(latest)} countries report {args.year}; '
        f'the rest keep their latest year from {args.oldest}. Entries are [year, TWh, population].",',
        f'  "world": [{args.year}, {world[0]:.3f}, {round(world[1])}],',
        '  "aliases": ' + json.dumps(aliases, sort_keys=True) + ',',
        '  "countries": {',
    ]
    entries = sorted(latest.items())
    for i, (code, (year, twh, population)) in enumerate(entries):
        comma = ',' if i + 1 < len(entries) else ''
        lines.append(f'    "{code}": [{year}, {twh:.3f}, {round(population)}]{comma}')
    lines += ['  }', '}']
    print('\n'.join(lines))


if __name__ == '__main__':
    main()
