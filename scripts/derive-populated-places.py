#!/usr/bin/env python3
"""Derive crates/tellegen-synth/data/populated_places.json, the populated
places the generator anchors buses on and sizes demand from.

usage: python3 -I scripts/derive-populated-places.py NE_PLACES_GEOJSON > \
           crates/tellegen-synth/data/populated_places.json

NE_PLACES_GEOJSON is ne_10m_populated_places_simple.geojson of Natural Earth
v5.1.2 (public domain),
https://raw.githubusercontent.com/nvkelso/natural-earth-vector/v5.1.2/geojson/ne_10m_populated_places_simple.geojson
It is third-party input: it is parsed as JSON, only the fields below are
read, and nothing in it is executed. Each place keeps `nameascii` (n), its
point's longitude and latitude rounded to four decimals (lo, la, about 11 m),
`pop_max` (p, with Natural Earth's -99 for unknown) and `adm0_a3` (c), the
code the demand and area tables are keyed by. Places are listed by `pop_max`,
largest first, in file order among equals.
"""
import argparse
import hashlib
import json
import math
import sys
from pathlib import Path


def place(feature):
    properties = feature.get('properties') or {}
    geometry = feature.get('geometry') or {}
    if geometry.get('type') != 'Point':
        raise ValueError(f'{properties.get("nameascii")!r} is not a point')
    lon, lat = (float(value) for value in geometry['coordinates'][:2])
    population = properties['pop_max']
    if not (math.isfinite(lon) and math.isfinite(lat)) or not isinstance(population, int):
        raise ValueError(f'{properties.get("nameascii")!r} has an invalid coordinate or pop_max')
    return {
        'n': str(properties['nameascii']),
        'lo': round(lon, 4),
        'la': round(lat, 4),
        'p': population,
        'c': str(properties['adm0_a3']),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('geojson', type=Path)
    args = parser.parse_args()
    raw = args.geojson.read_bytes()
    places = [place(feature) for feature in json.loads(raw)['features']]
    if not places:
        sys.exit('no populated places found')
    # sorted() is stable, so places of equal pop_max keep the file's order.
    places = sorted(places, key=lambda row: -row['p'])
    lines = [
        '{',
        '  "schema": "tellegen.synth-populated-places/1",',
        '  "source": "Natural Earth v5.1.2, 1:10m populated places (public domain)",',
        '  "source_url": "https://github.com/nvkelso/natural-earth-vector/blob/v5.1.2/geojson/ne_10m_populated_places_simple.geojson",',
        f'  "source_sha256": "{hashlib.sha256(raw).hexdigest()}",',
        '  "note": "Name (nameascii), longitude and latitude to four decimals, pop_max and adm0_a3 of every place, largest pop_max first, derived by scripts/derive-populated-places.py.",',
        '  "places": [',
    ]
    for i, row in enumerate(places):
        comma = ',' if i + 1 < len(places) else ''
        # One unindented row per line: diffs stay readable and the file the
        # browser downloads stays within 2% of the bare rows.
        lines.append(f'{json.dumps(row, separators=(",", ":"), ensure_ascii=False)}{comma}')
    lines += ['  ]', '}']
    sys.stdout.buffer.write(('\n'.join(lines) + '\n').encode('utf-8'))


if __name__ == '__main__':
    main()
