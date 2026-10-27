#!/usr/bin/env python3
"""Write the country-scale sweep specs from a country outline layer.

usage: python3 -I docs/evidence/synth/regions.py GEOJSON [--outline 50m|app-110m] [--out DIR]

Two outlines:

- 50m (the default): ne_50m_admin_0_countries.geojson of Natural Earth
  v5.1.2 (public domain), SOURCE_URL below. Its SHA-256 must be
  SOURCE_SHA256, so the specs are the ones the committed results came from.
  Countries are keyed by ADM0_A3 and coordinates rounded to 1e-4 degrees
  (about 11 m).
- app-110m: the MapToGrid app's own selection layer,
  static/data/countries-110m.geojson (Natural Earth 1:110m, properties
  `name` and `iso_a3`). Countries are keyed by `iso_a3` and the geometry is
  copied unrounded, so a spec is exactly the region the app sends; the
  request also states the app's defaults (seed 2026 is set by the sweep).
  The file's SHA-256 goes into every spec's description.

Either file is third-party input: it is parsed as JSON, only Polygon and
MultiPolygon coordinates are copied, and nothing in it is executed. Each
spec runs the default request on one whole country, plus one bounding box
around Texas. A country the layer lacks is reported and skipped. The specs
are not committed; this script regenerates them.
"""
import argparse
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
SOURCE_URL = ('https://raw.githubusercontent.com/nvkelso/natural-earth-vector/v5.1.2/'
              'geojson/ne_50m_admin_0_countries.geojson')
SOURCE_SHA256 = '3e458fc036ad0a66411f2c1e6cac49c5d7bfb81cb1123bc513b22511a2b7fdeb'
COUNTRIES = ['PRT', 'ESP', 'DEU', 'FRA', 'GBR', 'USA', 'IND', 'CHN', 'RUS', 'BRA', 'JPN',
             'AUS', 'ZAF', 'NGA', 'EGY', 'CHL', 'IDN']
TEXAS_BOX = (-106.65, 25.84, -93.51, 36.50)


def rounded(coords):
    if isinstance(coords, list) and coords and isinstance(coords[0], (int, float)):
        return [round(float(coords[0]), 4), round(float(coords[1]), 4)]
    return [rounded(c) for c in coords]


# The request the MapToGrid app sends for an untouched Generation panel
# (src/lib/engine/request.ts): the engine's defaults, stated.
APP_REQUEST = {'source': 'synthetic', 'density': 'high', 'fidelity': 'high',
               'demand': {'kind': 'population'}}


def spec(spec_id, description, geometry, label, extra=None):
    return {
        'id': spec_id,
        'description': description,
        'seeds': [1],
        'request': dict({'region': {'geometry': geometry, 'label': label}}, **(extra or {})),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('geojson', type=Path)
    parser.add_argument('--outline', choices=['50m', 'app-110m'], default='50m')
    parser.add_argument('--out', type=Path, default=HERE / 'sweep')
    args = parser.parse_args()
    raw = args.geojson.read_bytes()
    source = hashlib.sha256(raw).hexdigest()
    app = args.outline == 'app-110m'
    if not app and source != SOURCE_SHA256:
        raise SystemExit(f'{args.geojson} has sha256 {source}, expected {SOURCE_SHA256} ({SOURCE_URL})')
    features = json.loads(raw)['features']
    by_code = {}
    for feature in features:
        props = feature.get('properties') or {}
        code = props.get('iso_a3') if app else props.get('ADM0_A3')
        geometry = feature.get('geometry') or {}
        if code in COUNTRIES and geometry.get('type') in ('Polygon', 'MultiPolygon'):
            name = props.get('name') if app else (props.get('NAME_EN') or props.get('NAME'))
            by_code[code] = (name or code, geometry)
    missing = [c for c in COUNTRIES if c not in by_code]
    if missing and not app:
        raise SystemExit(f'not in the file: {missing}')
    for code in missing:
        print(f'{code} is not in the layer; skipped')
    layer = (f"the MapToGrid app's 1:110m layer (sha256 {source})" if app
             else f'Natural Earth 1:50m admin-0 polygon (source sha256 {source})')
    extra = APP_REQUEST if app else None
    args.out.mkdir(parents=True, exist_ok=True)
    written = 0
    for code in COUNTRIES:
        if code not in by_code:
            continue
        name, geometry = by_code[code]
        coords = geometry['coordinates'] if app else rounded(geometry['coordinates'])
        text = json.dumps(spec(code.lower(), f'{name} ({code}), the whole country: {layer}.',
                               {'type': geometry['type'], 'coordinates': coords}, name, extra),
                          separators=(',', ':'))
        (args.out / f'{code.lower()}.json').write_text(text + '\n')
        written += 1
    west, south, east, north = TEXAS_BOX
    box = {'type': 'Polygon', 'coordinates': [[[west, south], [east, south], [east, north],
                                               [west, north], [west, south]]]}
    text = json.dumps(spec('texas-box', 'A bounding box around Texas; it also takes in parts of '
                           'Mexico, New Mexico, Oklahoma and Louisiana.', box, 'Texas box', extra),
                      indent=2)
    (args.out / 'texas-box.json').write_text(text + '\n')
    print(f'wrote {written + 1} specs to {args.out}')


if __name__ == '__main__':
    main()
