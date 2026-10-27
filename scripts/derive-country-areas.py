#!/usr/bin/env python3
"""Derive crates/tellegen-synth/data/country_areas.json, the area of every
country the national demand basis spreads its unlisted demand over.

usage: python3 -I scripts/derive-country-areas.py NE_ADMIN0_GEOJSON > \
           crates/tellegen-synth/data/country_areas.json

NE_ADMIN0_GEOJSON is ne_50m_admin_0_countries.geojson of Natural Earth
v5.1.2 (public domain),
https://raw.githubusercontent.com/nvkelso/natural-earth-vector/v5.1.2/geojson/ne_50m_admin_0_countries.geojson
It is third-party input: it is parsed as JSON, only Polygon and MultiPolygon
coordinates are read, and nothing in it is executed. Areas are keyed by
ADM0_A3, the code the places table carries, and computed with the generator's
own spherical ring area (geom::ring_area_km2: Chamberlain and Duquette 2007,
R = 6371 km), outer rings minus holes, so a whole-country region measures
the same area the table records.
"""
import argparse
import hashlib
import json
import math
import re
import sys
from pathlib import Path

EARTH_RADIUS_KM = 6371.0
CODE = re.compile(r'[A-Z]{3}')


def ring_area_km2(ring):
    if len(ring) < 3:
        return 0.0
    total = 0.0
    for i, (lon1, lat1) in enumerate(ring):
        lon2, lat2 = ring[(i + 1) % len(ring)]
        total += math.radians(lon2 - lon1) * (2.0 + math.sin(math.radians(lat1)) + math.sin(math.radians(lat2)))
    return abs(total * EARTH_RADIUS_KM * EARTH_RADIUS_KM / 2.0)


def ring(coords):
    out = []
    for point in coords:
        lon, lat = float(point[0]), float(point[1])
        if not (math.isfinite(lon) and math.isfinite(lat)):
            raise ValueError('non-finite coordinate')
        out.append((lon, lat))
    # GeoJSON closes rings; the area sum does not need the repeated vertex.
    if len(out) > 1 and out[0] == out[-1]:
        out.pop()
    return out


def polygon_area(rings):
    if not rings:
        return 0.0
    area = ring_area_km2(ring(rings[0]))
    for hole in rings[1:]:
        area -= ring_area_km2(ring(hole))
    return max(area, 0.0)


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('geojson', type=Path)
    args = parser.parse_args()
    raw = args.geojson.read_bytes()
    areas = {}
    for feature in json.loads(raw)['features']:
        code = (feature.get('properties') or {}).get('ADM0_A3') or ''
        geometry = feature.get('geometry') or {}
        if not CODE.fullmatch(code):
            continue
        if geometry.get('type') == 'Polygon':
            polygons = [geometry['coordinates']]
        elif geometry.get('type') == 'MultiPolygon':
            polygons = geometry['coordinates']
        else:
            continue
        areas[code] = areas.get(code, 0.0) + sum(polygon_area(p) for p in polygons)
    if not areas:
        sys.exit('no admin-0 polygons found')
    lines = [
        '{',
        '  "schema": "tellegen.synth-country-areas/1",',
        '  "source": "Natural Earth v5.1.2, 1:50m admin-0 countries (public domain)",',
        '  "source_url": "https://github.com/nvkelso/natural-earth-vector/blob/v5.1.2/geojson/ne_50m_admin_0_countries.geojson",',
        f'  "source_sha256": "{hashlib.sha256(raw).hexdigest()}",',
        '  "note": "Spherical area of each ADM0_A3 country, outer rings minus holes, km2, derived by scripts/derive-country-areas.py.",',
        '  "areas_km2": {',
    ]
    entries = sorted(areas.items())
    for i, (code, area) in enumerate(entries):
        comma = ',' if i + 1 < len(entries) else ''
        lines.append(f'    "{code}": {round(area)}{comma}')
    lines += ['  }', '}']
    print('\n'.join(lines))


if __name__ == '__main__':
    main()
