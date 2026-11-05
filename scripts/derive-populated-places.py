#!/usr/bin/env python3
"""Derive crates/tellegen-synth/data/populated_places.json, the populated
places the generator anchors buses on and spreads demand over.

usage: python3 -I scripts/derive-populated-places.py CITIES5000_TXT COUNTRY_INFO_TXT > \
           crates/tellegen-synth/data/populated_places.json

CITIES5000_TXT is cities5000.txt from GeoNames' daily dump
(https://download.geonames.org/export/dump/cities5000.zip, unzipped): every
populated place of 5,000 people or more, about 70,000 worldwide.
COUNTRY_INFO_TXT is https://download.geonames.org/export/dump/countryInfo.txt,
which maps GeoNames' ISO 3166-1 alpha-2 codes to alpha-3 codes; four of them
are written as Natural Earth's ADM0_A3 (NATURAL_EARTH_CODES), the code the
area table is keyed by. GeoNames data is CC BY 4.0
(https://www.geonames.org/). Both files are third-party input: they are read
as text, every field used is validated, and nothing in them is executed.

GeoNames populations are those of the place itself, not of its metropolitan
area, so the places of a region sum to about its population (California's 715
places hold 38.5 of its 39 million people) without double counting suburbs.

Each row is [name, lon, lat, population, country] plus, in the United States,
the ISO 3166-2 code of the state ("US-CA"), from GeoNames' admin1 code, which
for the United States is the state's postal abbreviation. Coordinates keep
three decimals (about 110 m). Rows are listed by population, largest first,
in GeoNames id order among equals.
"""
import argparse
import hashlib
import json
import math
import re
import sys
from pathlib import Path

ISO2 = re.compile(r'[A-Z]{2}')
ISO3 = re.compile(r'[A-Z]{3}')
US_STATE = re.compile(r'[A-Z]{2}')
# GeoNames codes that are not ISO 3166-1 but name a country the tables know:
# Kosovo (XK), which GeoNames writes as XKX.
EXTRA_ISO3 = {'XK': 'XKX'}
# The area table is keyed by Natural Earth's ADM0_A3, which differs from ISO
# 3166-1 for these four; the places carry Natural Earth's code so every table
# lookup (national_demand.json aliases, country_areas.json) finds them.
NATURAL_EARTH_CODES = {'SSD': 'SDS', 'XKX': 'KOS', 'ESH': 'SAH', 'ALA': 'ALD'}


def country_codes(text):
    codes = {}
    for line in text.splitlines():
        if not line or line.startswith('#'):
            continue
        fields = line.split('\t')
        if len(fields) < 2 or not ISO2.fullmatch(fields[0]) or not ISO3.fullmatch(fields[1]):
            raise ValueError(f'countryInfo line {line[:60]!r} lacks ISO codes')
        codes[fields[0]] = fields[1]
    codes.update({k: v for k, v in EXTRA_ISO3.items() if k not in codes})
    return codes


def place(line, codes):
    fields = line.split('\t')
    if len(fields) < 19:
        raise ValueError(f'GeoNames row {line[:60]!r} has {len(fields)} fields, expected 19')
    geonameid, asciiname = fields[0], fields[2]
    lat, lon, cc, admin1, population = fields[4], fields[5], fields[8], fields[10], fields[14]
    lon, lat = float(lon), float(lat)
    population = int(population or 0)
    if not (math.isfinite(lon) and math.isfinite(lat) and -180 <= lon <= 180 and -90 <= lat <= 90):
        raise ValueError(f'{asciiname!r} ({geonameid}) has an invalid coordinate')
    if cc not in codes:
        raise ValueError(f'{asciiname!r} ({geonameid}) has unknown country code {cc!r}')
    iso3 = codes[cc]
    row = [asciiname or fields[1], round(lon, 3), round(lat, 3), population, NATURAL_EARTH_CODES.get(iso3, iso3)]
    if cc == 'US' and US_STATE.fullmatch(admin1):
        row.append(f'US-{admin1}')
    return int(geonameid), row


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('cities', type=Path)
    parser.add_argument('country_info', type=Path)
    args = parser.parse_args()
    raw = args.cities.read_bytes()
    info = args.country_info.read_bytes()
    codes = country_codes(info.decode('utf-8'))
    rows = [place(line, codes) for line in raw.decode('utf-8').splitlines() if line.strip()]
    if not rows:
        sys.exit('no populated places found')
    rows.sort(key=lambda r: (-r[1][3], r[0]))
    lines = [
        '{',
        '  "schema": "tellegen.synth-populated-places/2",',
        '  "source": "GeoNames cities5000 (CC BY 4.0, https://www.geonames.org/)",',
        '  "source_url": "https://download.geonames.org/export/dump/cities5000.zip",',
        f'  "source_sha256": "{hashlib.sha256(raw).hexdigest()}",',
        f'  "country_info_sha256": "{hashlib.sha256(info).hexdigest()}",',
        '  "note": "Rows are [asciiname, longitude, latitude (three decimals), population, ISO 3166-1 alpha-3] '
        'and, in the United States, the ISO 3166-2 state code; largest population first. Derived by '
        'scripts/derive-populated-places.py.",',
        '  "places": [',
    ]
    for i, (_, row) in enumerate(rows):
        comma = ',' if i + 1 < len(rows) else ''
        lines.append(f'{json.dumps(row, separators=(",", ":"), ensure_ascii=True)}{comma}')
    lines += ['  ]', '}']
    sys.stdout.buffer.write(('\n'.join(lines) + '\n').encode('utf-8'))


if __name__ == '__main__':
    main()
