"""Realism metrics for a geo-referenced transmission network, computed the same
way for the California Test System (CATS) and for a generated grid.

The loaders turn either source into one `Net` (buses with coordinates, branches
with route lengths, generators with a fuel class). `measure(net)` returns a flat
dict of metrics. Nothing here knows which network is the reference: compare.py
holds the tolerances and does the scoring.

The comparison graph, used by every topology metric:
- the largest connected component (by bus count);
- degree-2 buses with no load and no machine whose two branches are lines at
  the same voltage are contracted into one branch, repeatedly. CATS digitises
  lines as straight segments with a bus at every vertex (5,699 of its 8,870
  buses are such "added nodes"), so without this step bus counts and degrees
  would measure digitisation, not topology;
- sites: buses within SITE_RADIUS_KM of one another (one substation holding
  several voltage levels is one site).
"""
from __future__ import annotations

import csv
import json
import math
import re
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
from scipy.optimize import linprog
from scipy.sparse import coo_matrix, csr_matrix
from scipy.sparse.csgraph import connected_components, minimum_spanning_tree
from scipy.spatial import Delaunay, cKDTree
from scipy.stats import spearmanr

EARTH_KM = 6371.0088
CLASSES_KV = (66.0, 115.0, 230.0, 500.0)
SITE_RADIUS_KM = 0.3
# Voltage bands of Birchfield et al. 2017 (Table 5) plus the subtransmission
# band CATS models at 66 kV.
BANDS = {'lv': (0.0, 100.0), 'mv': (100.0, 200.0), 'hv': (200.0, 2000.0)}
FUELS = ('gas', 'oil', 'coal', 'nuclear', 'hydro', 'wind', 'solar', 'storage', 'other')


@dataclass
class Net:
    base_mva: float
    # Bus arrays, index = position.
    bus_id: np.ndarray
    kv: np.ndarray
    lat: np.ndarray
    lon: np.ndarray
    pd: np.ndarray
    qd: np.ndarray
    # Branch arrays (in service only), endpoints as bus positions.
    f: np.ndarray
    t: np.ndarray
    r: np.ndarray
    x: np.ndarray
    b: np.ndarray
    rate: np.ndarray
    is_tx: np.ndarray
    length_km: np.ndarray
    # Generator arrays (in service only).
    g_bus: np.ndarray
    g_pmax: np.ndarray
    g_pmin: np.ndarray
    g_qmax: np.ndarray
    g_cost: np.ndarray  # $/MWh at mid output, for the DC OPF
    g_fuel: list
    g_is_condenser: np.ndarray
    g_is_import: np.ndarray
    # Optional: line routes as lists of (lon, lat), one per branch, for the
    # line density metric. None falls back to the straight segment.
    routes: list | None = None
    # Optional: which buses stand at a site, for the siting metrics: not a
    # generated grid's tap junctions, nor CATS's added nodes. None counts
    # every bus.
    sited: np.ndarray | None = None
    notes: dict = field(default_factory=dict)


# --------------------------------------------------------------------------
# Geometry helpers
# --------------------------------------------------------------------------

def haversine_km(lat1, lon1, lat2, lon2):
    p1, p2 = np.radians(lat1), np.radians(lat2)
    dp, dl = p2 - p1, np.radians(np.asarray(lon2) - np.asarray(lon1))
    a = np.sin(dp / 2) ** 2 + np.cos(p1) * np.cos(p2) * np.sin(dl / 2) ** 2
    return 2 * EARTH_KM * np.arcsin(np.sqrt(np.clip(a, 0, 1)))


def route_km(coords):
    if len(coords) < 2:
        return 0.0
    c = np.asarray(coords, dtype=float)
    return float(haversine_km(c[:-1, 1], c[:-1, 0], c[1:, 1], c[1:, 0]).sum())


def project(lat, lon, lat0, lon0=0.0):
    """Equirectangular km about (lat0, lon0): accurate to well under 1% over a
    state. Two networks compared must share lat0 and lon0."""
    k = math.radians(1) * EARTH_KM
    return np.column_stack([(np.asarray(lon) - lon0) * k * math.cos(math.radians(lat0)),
                            np.asarray(lat) * k])


def kv_class(kv):
    kv = np.asarray(kv, dtype=float)
    cls = np.array(CLASSES_KV)
    idx = np.abs(np.log(np.maximum(kv, 1.0))[:, None] - np.log(cls)[None, :]).argmin(axis=1)
    return cls[idx]


def band_of(kv):
    for name, (lo, hi) in BANDS.items():
        if lo <= kv < hi:
            return name
    return 'hv'


# --------------------------------------------------------------------------
# Loaders
# --------------------------------------------------------------------------

def parse_matpower(path):
    """The numeric matrices of a MATPOWER case file: baseMVA, bus, gen, branch,
    gencost. Comments and the trailing semicolons are dropped; nothing in the
    file is evaluated."""
    text = Path(path).read_text()
    base = float(re.search(r'mpc\.baseMVA\s*=\s*([0-9.eE+-]+)', text).group(1))
    out = {'baseMVA': base}
    for name in ('bus', 'gen', 'branch', 'gencost'):
        m = re.search(r'mpc\.' + name + r'\s*=\s*\[(.*?)\];', text, re.S)
        if not m:
            out[name] = []
            continue
        rows = []
        for raw in m.group(1).splitlines():
            line = raw.split('%', 1)[0].strip().rstrip(';').strip()
            if not line:
                continue
            rows.append([float(v) for v in line.replace(',', ' ').split()])
        out[name] = rows
    return out


def classify_fuel(text):
    s = (text or '').lower()
    if 'condenser' in s:
        return 'condenser'
    if 'import' in s:
        return 'import'
    for key, fuel in (('natural gas', 'gas'), ('gas', 'gas'), ('petroleum', 'oil'), ('oil', 'oil'),
                      ('coal', 'coal'), ('nuclear', 'nuclear'), ('pumped', 'storage'),
                      ('batter', 'storage'), ('storage', 'storage'), ('hydro', 'hydro'),
                      ('wind', 'wind'), ('solar', 'solar'), ('photovoltaic', 'solar')):
        if key in s:
            return fuel
    return 'other'


def _assemble(case, coords, fuels, lengths, routes=None, notes=None):
    base = case['baseMVA']
    bus = np.array(case['bus'])
    ids = bus[:, 0].astype(int)
    pos = {b: i for i, b in enumerate(ids)}
    lat = np.array([coords[b][0] for b in ids])
    lon = np.array([coords[b][1] for b in ids])
    br = np.array(case['branch'])
    live = br[:, 10] > 0
    f = np.array([pos[int(v)] for v in br[live, 0]])
    t = np.array([pos[int(v)] for v in br[live, 1]])
    kv = bus[:, 9]
    ratio = br[live, 8]
    is_tx = (ratio != 0) | (np.abs(kv[f] - kv[t]) > 0.5)
    seg = haversine_km(lat[f], lon[f], lat[t], lon[t])
    length = np.where(np.isfinite(lengths[live]) & (lengths[live] > 0), lengths[live], seg) \
        if lengths is not None else seg
    gen = np.array(case['gen'])
    gl = gen[:, 7] > 0
    gidx = np.nonzero(gl)[0]
    g_fuel = [fuels[i] for i in gidx]
    cost_rows = case.get('gencost') or []
    lin = np.zeros(len(gen))
    for i, row in enumerate(cost_rows[:len(gen)]):
        if int(row[0]) == 2:
            nc = int(row[3])
            c = list(reversed(row[4:4 + nc]))
            c1 = c[1] if len(c) > 1 else 0.0
            c2 = c[2] if len(c) > 2 else 0.0
            lin[i] = c1 + c2 * gen[i, 8]  # marginal cost at half output: c1 + 2 c2 (pmax/2)
    return Net(
        base_mva=base, bus_id=ids, kv=kv, lat=lat, lon=lon, pd=bus[:, 2], qd=bus[:, 3],
        f=f, t=t, r=br[live, 2], x=br[live, 3], b=br[live, 4], rate=br[live, 5],
        is_tx=is_tx, length_km=length,
        g_bus=np.array([pos[int(v)] for v in gen[gl, 0]]), g_pmax=gen[gl, 8], g_pmin=gen[gl, 9],
        g_qmax=gen[gl, 3], g_cost=lin[gl], g_fuel=g_fuel,
        g_is_condenser=np.array([fu == 'condenser' or (p <= 0 and q > 0)
                                 for fu, p, q in zip(g_fuel, gen[gl, 8], gen[gl, 3], strict=True)]),
        g_is_import=np.array([fu == 'import' for fu in g_fuel]),
        routes=[routes[i] for i in np.nonzero(live)[0]] if routes is not None else None,
        notes=notes or {},
    )


def load_cats(directory):
    """CATS as staged by scripts/stage-data.sh: CaliforniaTestSystem.m,
    CATS_buses.csv (locations, and Type: Substation or AddedNode),
    CATS_gens.csv and CATS_lines.json."""
    d = Path(directory)
    case = parse_matpower(d / 'CaliforniaTestSystem.m')
    coords, kinds = {}, {}
    with open(d / 'CATS_buses.csv', newline='') as fh:
        for row in csv.DictReader(fh):
            coords[int(row['bus_i'])] = (float(row['Lat']), float(row['Lon']))
            kinds[int(row['bus_i'])] = row['Type'].strip("' ")
    fuels = []
    with open(d / 'CATS_gens.csv', newline='') as fh:
        for row in csv.DictReader(fh):
            fuels.append(classify_fuel(row['FuelType']))
    lengths = np.full(len(case['branch']), np.nan)
    routes = [None] * len(case['branch'])
    lines_path = d / 'CATS_lines.json'
    if lines_path.exists():
        feats = json.loads(lines_path.read_text())['features']
        by_pair = defaultdict(list)
        for feat in feats:
            p = feat['properties']
            geom = feat['geometry']
            parts = geom['coordinates'] if geom['type'] == 'MultiLineString' else [geom['coordinates']]
            by_pair[(int(p['f_bus']), int(p['t_bus']))].append(
                (sum(route_km(part) for part in parts), [c for part in parts for c in part]))
        for i, row in enumerate(case['branch']):
            key = (int(row[0]), int(row[1]))
            if by_pair.get(key):
                lengths[i], routes[i] = by_pair[key].pop(0)
    for i, rt in enumerate(routes):
        if rt is None:
            a, b = coords[int(case['branch'][i][0])], coords[int(case['branch'][i][1])]
            routes[i] = [(a[1], a[0]), (b[1], b[0])]
    net = _assemble(case, coords, fuels, lengths, routes, notes={'source': 'CATS'})
    # Its substations stand at sites; its added nodes are line junctions.
    net.sited = np.array([kinds[int(b)] == 'Substation' for b in net.bus_id], dtype=bool)
    return net


def load_generated(directory):
    """A `tellegen generate` output directory: network.pio.json (buses with
    locations and kind, branches with routes) and report.json (per-element
    fuel, condenser flag, route length and transformer flag)."""
    d = Path(directory)
    data = json.loads((d / 'network.pio.json').read_text())['value']['data']
    report = json.loads((d / 'report.json').read_text())
    base = float(data.get('base_mva') or 100.0)
    buses = data['buses']
    ids = np.array([int(b['id']) for b in buses])
    pos = {b: i for i, b in enumerate(ids)}
    loc = [b.get('location') or {} for b in buses]
    lat = np.array([float(p.get('y', p.get('lat', 0.0))) for p in loc])
    lon = np.array([float(p.get('x', p.get('lon', 0.0))) for p in loc])
    kv = np.array([float(b['base_kv']) for b in buses])
    rep_bus = {e['uid']: e for e in report['elements']['buses']}
    kind = [((b.get('extras') or {}).get('tellegen_synth') or rep_bus.get(b.get('uid')) or {}).get('kind')
            for b in buses]
    pd = np.zeros(len(ids))
    qd = np.zeros(len(ids))
    for ld in data.get('loads') or []:
        if ld.get('in_service', True):
            pd[pos[int(ld['bus'])]] += float(ld['p'])
            qd[pos[int(ld['bus'])]] += float(ld['q'])
    rep_br = {e['uid']: e for e in report['elements']['branches']}
    f, t, r, x, b, rate, is_tx, length, routes = [], [], [], [], [], [], [], [], []
    for br in data['branches']:
        if not br.get('in_service', True):
            continue
        ex = (br.get('extras') or {}).get('tellegen_synth') or rep_br.get(br.get('uid')) or {}
        u, v = pos[int(br['from'])], pos[int(br['to'])]
        f.append(u)
        t.append(v)
        r.append(float(br['r']))
        x.append(float(br['x']))
        b.append(float(br.get('b') or 0.0))
        rate.append(float(br.get('rate_a') or 0.0))
        is_tx.append(bool(ex.get('is_transformer')) or float(br.get('tap') or 0.0) != 0.0
                     or abs(kv[u] - kv[v]) > 0.5)
        pts = [(float(p.get('x', p.get('lon'))), float(p.get('y', p.get('lat'))))
               for p in (br.get('route') or [])]
        if len(pts) < 2:
            pts = [(lon[u], lat[u]), (lon[v], lat[v])]
        routes.append(pts)
        length.append(float(ex.get('length_km') or route_km(pts)))
    rep_gen = {e['uid']: e for e in report['elements']['generators']}
    g_bus, g_pmax, g_pmin, g_qmax, g_cost, g_fuel, g_cond = [], [], [], [], [], [], []
    for g in data['generators']:
        if not g.get('in_service', True):
            continue
        e = rep_gen.get(g.get('uid')) or {}
        fuel = classify_fuel(str(e.get('fuel') or g.get('energy_source') or ''))
        g_bus.append(pos[int(g['bus'])])
        g_pmax.append(float(g['pmax']))
        g_pmin.append(float(g.get('pmin') or 0.0))
        g_qmax.append(float(g.get('qmax') or 0.0))
        coeffs = (g.get('cost') or {}).get('coeffs') or []
        c2, c1 = (coeffs[0], coeffs[1]) if len(coeffs) >= 3 else (0.0, coeffs[-2] if len(coeffs) >= 2 else 0.0)
        g_cost.append(c1 + c2 * float(g['pmax']))
        g_fuel.append(fuel)
        g_cond.append(bool(e.get('condenser')) or (float(g['pmax']) <= 0 and float(g.get('qmax') or 0) > 0))
    arr = np.array
    return Net(
        base_mva=base, bus_id=ids, kv=kv, lat=lat, lon=lon, pd=pd, qd=qd,
        f=arr(f, dtype=int), t=arr(t, dtype=int), r=arr(r), x=arr(x), b=arr(b), rate=arr(rate),
        is_tx=arr(is_tx, dtype=bool), length_km=arr(length),
        g_bus=arr(g_bus, dtype=int), g_pmax=arr(g_pmax), g_pmin=arr(g_pmin), g_qmax=arr(g_qmax),
        g_cost=arr(g_cost), g_fuel=g_fuel, g_is_condenser=arr(g_cond, dtype=bool),
        g_is_import=np.zeros(len(g_bus), dtype=bool), routes=routes,
        sited=np.array([k != 'tap' for k in kind], dtype=bool),
        notes={'source': str(d), 'dc_opf': report.get('dc_opf', {}).get('status'),
               'ac_pf': {k: report.get('ac_pf', {}).get(k) for k in
                         ('status', 'vm_min', 'vm_max', 'losses_mw', 'buses_outside_emergency_band')},
               'context': {k: report['context'].get(k) for k in
                           ('peak_demand_mw', 'target_bus_count', 'voltage_levels_kv', 'demand_basis',
                            'voltage_basis', 'voltage_country')}},
    )


# --------------------------------------------------------------------------
# Graph preparation
# --------------------------------------------------------------------------

def main_component(net):
    n = len(net.kv)
    adj = coo_matrix((np.ones(len(net.f)), (net.f, net.t)), shape=(n, n))
    ncomp, labels = connected_components(adj, directed=False)
    counts = np.bincount(labels)
    main = counts.argmax()
    keep = labels == main
    load_share = float(net.pd[keep].sum() / max(net.pd.sum(), 1e-9))
    return keep, ncomp, load_share


def reduce_series(net, keep):
    """Contract no-injection degree-2 buses between two same-kV lines. Returns
    (bus mask, list of reduced edges (u, v, length_km, is_tx, kv))."""
    n = len(net.kv)
    inj = np.zeros(n, dtype=bool)
    inj[np.abs(net.pd) > 1e-6] = True
    inj[net.g_bus] = True
    edges = {}
    inc = defaultdict(set)
    for k in range(len(net.f)):
        u, v = int(net.f[k]), int(net.t[k])
        if not (keep[u] and keep[v]) or u == v:
            continue
        edges[k] = [u, v, float(net.length_km[k]), bool(net.is_tx[k]), float(max(net.kv[u], net.kv[v]))]
        inc[u].add(k)
        inc[v].add(k)
    alive = keep.copy()
    next_id = max(edges) + 1 if edges else 0
    stack = [u for u in range(n) if alive[u] and len(inc[u]) == 2 and not inj[u]]
    while stack:
        u = stack.pop()
        if not alive[u] or inj[u] or len(inc[u]) != 2:
            continue
        k1, k2 = tuple(inc[u])
        e1, e2 = edges[k1], edges[k2]
        if e1[3] or e2[3]:
            continue
        a = e1[1] if e1[0] == u else e1[0]
        b = e2[1] if e2[0] == u else e2[0]
        if a == b or abs(net.kv[a] - net.kv[u]) > 0.5 or abs(net.kv[b] - net.kv[u]) > 0.5:
            continue
        for kk, w in ((k1, a), (k2, b)):
            inc[w].discard(kk)
            del edges[kk]
        inc[u].clear()
        alive[u] = False
        edges[next_id] = [a, b, e1[2] + e2[2], False, float(net.kv[u])]
        inc[a].add(next_id)
        inc[b].add(next_id)
        next_id += 1
        for w in (a, b):
            if len(inc[w]) == 2 and not inj[w]:
                stack.append(w)
    return alive, list(edges.values())


def sites(net, mask):
    """Group buses within SITE_RADIUS_KM; returns site label per bus (-1 off mask)."""
    idx = np.nonzero(mask)[0]
    lat0 = float(np.mean(net.lat[idx]))
    xy = project(net.lat[idx], net.lon[idx], lat0)
    tree = cKDTree(xy)
    pairs = tree.query_pairs(SITE_RADIUS_KM, output_type='ndarray')
    m = len(idx)
    adj = coo_matrix((np.ones(len(pairs)), (pairs[:, 0], pairs[:, 1])), shape=(m, m)) if len(pairs) \
        else coo_matrix((m, m))
    _, lab = connected_components(adj, directed=False)
    out = np.full(len(net.kv), -1)
    out[idx] = lab
    return out


# --------------------------------------------------------------------------
# DC OPF (one LP, the same for every network)
# --------------------------------------------------------------------------

def dc_opf(net, keep):
    """DC OPF on the main component with linear costs, in flow form so the
    matrix stays well scaled whatever the reactances:
        x_l f_l - (theta_f - theta_t) base = 0,   |f_l| <= rate_l,
        sum_g pg - sum_out f + sum_in f = pd  at every bus.
    Returns (|flow|/rate for rated branches, status) or (None, message)."""
    from scipy.sparse import hstack, vstack
    buses = np.nonzero(keep)[0]
    pos = -np.ones(len(net.kv), dtype=int)
    pos[buses] = np.arange(len(buses))
    br = np.nonzero(keep[net.f] & keep[net.t] & (net.f != net.t) & (np.abs(net.x) > 0))[0]
    gens = np.nonzero(keep[net.g_bus] & (net.g_pmax > 0))[0]
    nb, ng, nl = len(buses), len(gens), len(br)
    fb, tb = pos[net.f[br]], pos[net.t[br]]
    base = net.base_mva
    # Variable order: theta (nb), pg (ng), flow (nl).
    ar = np.arange(nl)
    K = csr_matrix((np.concatenate([-np.ones(nl), np.ones(nl)]),
                    (np.concatenate([fb, tb]), np.concatenate([ar, ar]))), shape=(nb, nl))
    G = csr_matrix((np.ones(ng), (pos[net.g_bus[gens]], np.arange(ng))), shape=(nb, ng))
    balance = hstack([csr_matrix((nb, nb)), G, K])
    A = csr_matrix((np.concatenate([-np.full(nl, base), np.full(nl, base)]),
                    (np.concatenate([ar, ar]), np.concatenate([fb, tb]))), shape=(nl, nb))
    ohm = hstack([A, csr_matrix((nl, ng)), csr_matrix((net.x[br], (ar, ar)), shape=(nl, nl))])
    slack_bus = int(np.argmax(np.bincount(pos[net.g_bus[gens]], weights=net.g_pmax[gens], minlength=nb)))
    slack = csr_matrix(([1.0], ([0], [slack_bus])), shape=(1, nb + ng + nl))
    A_eq = vstack([balance, ohm, slack]).tocsr()
    b_eq = np.concatenate([net.pd[buses], np.zeros(nl), [0.0]])
    rate = net.rate[br]
    fbounds = [(-r, r) if r > 0 else (None, None) for r in rate]
    bounds = [(None, None)] * nb + list(zip(np.maximum(net.g_pmin[gens], 0), net.g_pmax[gens], strict=True)) + fbounds
    c = np.concatenate([np.zeros(nb), net.g_cost[gens] + 1e-3, np.zeros(nl)])
    res = linprog(c, A_eq=A_eq, b_eq=b_eq, bounds=bounds, method='highs')
    if res.status != 0:
        return None, res.message
    flow = res.x[nb + ng:]
    rated = rate > 0
    return np.abs(flow[rated]) / rate[rated], 'optimal'


# --------------------------------------------------------------------------
# Measurement
# --------------------------------------------------------------------------

def _shares(values, keys):
    tot = sum(values.values()) or 1.0
    return {k: values.get(k, 0.0) / tot for k in keys}


def _degree_pmf(deg):
    bins = Counter(min(int(d), 10) for d in deg)
    n = max(len(deg), 1)
    return {str(k): bins.get(k, 0) / n for k in range(1, 11)}


def _band_topology(site_xy, site_kv_max, site_edges, band):
    """Lines/sites, line length/MST and Delaunay neighbour shares for the site
    network at one voltage band."""
    lo, hi = BANDS[band]
    in_band = {s for s, levels in site_kv_max.items() if any(lo <= kv < hi for kv in levels)}
    edges = {(min(a, b), max(a, b)) for a, b, kv in site_edges
             if lo <= kv < hi and a in in_band and b in in_band and a != b}
    nodes = sorted(in_band)
    out = {'sites': len(nodes), 'lines': len(edges)}
    if len(nodes) < 10 or not edges:
        return out
    idx = {s: i for i, s in enumerate(nodes)}
    xy = np.array([site_xy[s] for s in nodes])
    out['lines_per_site'] = len(edges) / len(nodes)
    length = sum(float(np.linalg.norm(xy[idx[a]] - xy[idx[b]])) for a, b in edges)
    tri = Delaunay(xy, qhull_options='QJ')
    dn = defaultdict(set)
    for simplex in tri.simplices:
        for i in range(3):
            for j in range(i + 1, 3):
                a, b = int(simplex[i]), int(simplex[j])
                dn[a].add(b)
                dn[b].add(a)
    # Euclidean MST over the Delaunay edges (the MST is a subgraph of it).
    rows, cols, w = [], [], []
    for a, nb in dn.items():
        for b in nb:
            if a < b:
                rows.append(a)
                cols.append(b)
                w.append(float(np.linalg.norm(xy[a] - xy[b])) + 1e-9)
    mst = minimum_spanning_tree(coo_matrix((w, (rows, cols)), shape=(len(nodes), len(nodes))))
    out['length_over_mst'] = length / float(mst.sum())
    hops = Counter()
    for a, b in edges:
        ia, ib = idx[a], idx[b]
        if ib in dn[ia]:
            hops[1] += 1
            continue
        second = set().union(*(dn[c] for c in dn[ia]))
        hops[2 if ib in second else 3] += 1
    tot = sum(hops.values())
    out['delaunay_1'] = hops[1] / tot
    out['delaunay_2'] = hops[2] / tot
    out['delaunay_3plus'] = hops[3] / tot
    return out


def _cell_values(points_xy, weights, cell_km, origin, shape):
    ij = np.floor((points_xy - origin) / cell_km).astype(int)
    ok = (ij[:, 0] >= 0) & (ij[:, 1] >= 0) & (ij[:, 0] < shape[0]) & (ij[:, 1] < shape[1])
    grid = np.zeros(shape)
    np.add.at(grid, (ij[ok, 0], ij[ok, 1]), weights[ok])
    return grid


def _route_samples(net, keep, lat0, lon0, step_km=1.0):
    pts, w = [], []
    for k in range(len(net.f)):
        if net.is_tx[k] or not (keep[net.f[k]] and keep[net.t[k]]):
            continue
        route = net.routes[k] if net.routes is not None else None
        if not route or len(route) < 2:
            route = [(net.lon[net.f[k]], net.lat[net.f[k]]), (net.lon[net.t[k]], net.lat[net.t[k]])]
        c = np.asarray(route, dtype=float)
        xy = project(c[:, 1], c[:, 0], lat0, lon0)
        for p, q in zip(xy[:-1], xy[1:], strict=True):
            seg = float(np.linalg.norm(q - p))
            m = max(1, int(math.ceil(seg / step_km)))
            s = (np.arange(m) + 0.5) / m
            pts.append(p[None, :] + s[:, None] * (q - p)[None, :])
            w.append(np.full(m, seg / m))
    if not pts:
        return np.zeros((0, 2)), np.zeros(0)
    return np.vstack(pts), np.concatenate(w)


def measure(net, frame=None, with_dc=True, places=None):
    """Every metric of the scorecard, as a flat dict. `frame` is the
    projection centre (lat0, lon0) the geographic metrics use, so that two
    networks compared share one projection; None takes this network's mean.
    `places` ((lon, lat, population) rows, `load_places`) adds the siting
    metrics."""
    keep, ncomp, load_share = main_component(net)
    alive, redges = reduce_series(net, keep)
    m = {'components': int(ncomp), 'main_load_share': load_share,
         'raw_buses': int(len(net.kv)), 'raw_branches': int(len(net.f))}

    # --- scale -------------------------------------------------------------
    m['peak_load_mw'] = float(net.pd.sum())
    real = ~net.g_is_condenser & ~net.g_is_import & (net.g_pmax > 0)
    m['nameplate_mw'] = float(net.g_pmax[real].sum())
    m['generators'] = int(real.sum())
    fuel_mw = defaultdict(float)
    for fu, p, ok in zip(net.g_fuel, net.g_pmax, real, strict=True):
        if ok:
            fuel_mw[fu if fu in FUELS else 'other'] += float(p)
    m['fuel_mix'] = _shares(fuel_mw, FUELS)
    m['reduced_buses'] = int(alive.sum())
    site = sites(net, alive)
    m['sites'] = int(len(set(site[alive])))
    cls = kv_class(net.kv)
    counts = Counter(cls[alive])
    m['bus_share'] = {str(int(c)): counts.get(c, 0) / max(alive.sum(), 1) for c in CLASSES_KV}
    line = ~net.is_tx & keep[net.f] & keep[net.t]
    km = defaultdict(float)
    for k in np.nonzero(line)[0]:
        km[str(int(kv_class([max(net.kv[net.f[k]], net.kv[net.t[k]])])[0]))] += float(net.length_km[k])
    m['circuit_km'] = {str(int(c)): km.get(str(int(c)), 0.0) for c in CLASSES_KV}
    n_tx = int((net.is_tx & keep[net.f] & keep[net.t]).sum())
    m['transformers'] = n_tx
    m['transformers_per_site'] = n_tx / max(m['sites'], 1)

    # --- topology (reduced simple graph) -----------------------------------
    nodes = np.nonzero(alive)[0]
    nid = -np.ones(len(net.kv), dtype=int)
    nid[nodes] = np.arange(len(nodes))
    simple = {(min(nid[u], nid[v]), max(nid[u], nid[v])) for u, v, *_ in redges if nid[u] != nid[v]}
    nbrs = defaultdict(set)
    for a, b in simple:
        nbrs[a].add(b)
        nbrs[b].add(a)
    deg = np.array([len(nbrs[i]) for i in range(len(nodes))])
    m['mean_degree'] = float(deg.mean()) if len(deg) else 0.0
    m['max_degree'] = int(deg.max()) if len(deg) else 0
    m['degree_pmf'] = _degree_pmf(deg)
    cc = []
    for i in range(len(nodes)):
        k = len(nbrs[i])
        if k < 2:
            cc.append(0.0)
            continue
        nb = list(nbrs[i])
        links = sum(1 for a in range(k) for b in range(a + 1, k) if nb[b] in nbrs[nb[a]])
        cc.append(2 * links / (k * (k - 1)))
    m['clustering'] = float(np.mean(cc)) if cc else 0.0
    rng = np.random.default_rng(7)
    sources = rng.choice(len(nodes), size=min(200, len(nodes)), replace=False)
    hops = []
    for s in sources:
        dist = {int(s): 0}
        frontier = [int(s)]
        while frontier:
            nxt = []
            for u in frontier:
                for v in nbrs[u]:
                    if v not in dist:
                        dist[v] = dist[u] + 1
                        nxt.append(v)
            frontier = nxt
        hops.append(np.mean(list(dist.values())))
    m['mean_hops'] = float(np.mean(hops)) if hops else 0.0

    # Site network by band.
    if frame is None:
        frame = (float(np.mean(net.lat[alive])), float(np.mean(net.lon[alive])))
    lat0, lon0 = frame
    xy_bus = project(net.lat, net.lon, lat0, lon0)
    site_xy, site_levels = {}, defaultdict(set)
    for s in set(site[alive]):
        members = np.nonzero(site == s)[0]
        site_xy[s] = xy_bus[members].mean(axis=0)
    for u in nodes:
        site_levels[site[u]].add(float(net.kv[u]))
    site_edges = [(site[u], site[v], kv) for u, v, _, is_tx, kv in redges if not is_tx]
    m['bands'] = {b: _band_topology(site_xy, site_levels, site_edges, b) for b in BANDS}

    # --- parameters --------------------------------------------------------
    params = defaultdict(lambda: defaultdict(list))
    for k in np.nonzero(line & (net.length_km >= 1.0))[0]:
        kv = max(net.kv[net.f[k]], net.kv[net.t[k]])
        c = str(int(kv_class([kv])[0]))
        zb = kv * kv / net.base_mva
        params[c]['x_ohm_km'].append(net.x[k] * zb / net.length_km[k])
        if net.r[k] > 0:
            params[c]['x_over_r'].append(net.x[k] / net.r[k])
        if net.rate[k] > 0:
            params[c]['rate_mva'].append(net.rate[k])
    m['line_params'] = {c: {k: [float(v) for v in vals] for k, vals in d.items()} for c, d in params.items()}
    tx = np.nonzero(net.is_tx & keep[net.f] & keep[net.t] & (net.rate > 0))[0]
    m['tx_x_own_base'] = [float(net.x[k] * net.rate[k] / net.base_mva) for k in tx]

    # --- operation -----------------------------------------------------------
    if with_dc:
        loading, status = dc_opf(net, keep)
        m['dc_status'] = status
        if loading is not None:
            m['dc_loading_p50'] = float(np.percentile(loading, 50))
            m['dc_loading_p90'] = float(np.percentile(loading, 90))
            m['dc_binding'] = float(np.mean(loading >= 0.99))

    # --- geography ----------------------------------------------------------
    m['site_xy'] = [[float(v) for v in site_xy[s]] for s in sorted(site_xy)]
    pts, w = _route_samples(net, keep, lat0, lon0)
    m['_route_samples'] = (pts, w)
    m['_load_xy'] = (xy_bus[keep], net.pd[keep])
    m['_frame'] = frame
    if places is not None:
        m['siting'] = siting(net, places)
    return m


# --------------------------------------------------------------------------
# Siting: where the buses stand against where people live
# --------------------------------------------------------------------------

# MapToGrid's California measure (tellegen#305): 0.5° cells, a sparse cell
# being one whose listed places hold fewer than 20,000 people, the ten most
# populous cells, and three boxes (west, south, east, north) running from
# dense to empty. Box areas are MapToGrid's: a flat box at its mid latitude.
SITING_CELL_DEG = 0.5
SITING_SPARSE_PEOPLE = 20_000.0
SITING_TOP_CELLS = 10
SITING_BOXES = {
    'la': (-118.7, 33.6, -117.0, 34.4),      # the Los Angeles basin
    'mojave': (-117.5, 34.6, -115.5, 35.6),  # the Mojave Desert
    'ne': (-121.5, 40.5, -120.0, 42.0),      # Modoc and Lassen counties
}


def load_places(path, subdivision):
    """(lon, lat, population) rows of the bundled places table whose ISO
    3166-2 subdivision (the row's sixth field) is `subdivision`."""
    rows = json.loads(Path(path).read_text())['places']
    pts = [(r[1], r[2], r[3]) for r in rows if len(r) > 5 and r[5] == subdivision]
    return np.array(pts, dtype=float).reshape(-1, 3)


def siting(net, places):
    """Where the sited buses stand against the places' population: the
    share in sparse cells, the share in the most populous cells, buses per
    1,000 km² in each box, and the ratio of the densest box to the emptiest."""
    on = net.sited if net.sited is not None else np.ones(len(net.kv), dtype=bool)
    lon, lat = net.lon[on], net.lat[on]
    cells = defaultdict(lambda: [0, 0.0])
    for key in zip(np.floor(lon / SITING_CELL_DEG).astype(int), np.floor(lat / SITING_CELL_DEG).astype(int),
                   strict=True):
        cells[key][0] += 1
    for x, y, p in places:
        cells[(math.floor(x / SITING_CELL_DEG), math.floor(y / SITING_CELL_DEG))][1] += p
    total = max(len(lon), 1)
    sparse = sum(b for b, p in cells.values() if p < SITING_SPARSE_PEOPLE)
    top = sorted(cells.values(), key=lambda c: -c[1])[:SITING_TOP_CELLS]
    density = {}
    for name, (w, s, e, n) in SITING_BOXES.items():
        count = int(np.sum((lon > w) & (lon < e) & (lat > s) & (lat < n)))
        area = (e - w) * 111.32 * math.cos(math.radians((s + n) / 2)) * (n - s) * 110.57
        density[name] = 1000.0 * count / area
    return {'buses': int(on.sum()), 'sparse_share': sparse / total,
            'top_share': sum(b for b, _ in top) / total, 'per_1000km2': density,
            'la_over_ne': density['la'] / density['ne'] if density['ne'] > 0 else None}


def gridded(m, origin, shape, cell_km, which):
    if which == 'lines':
        pts, w = m['_route_samples']
    else:
        pts, w = m['_load_xy']
    return _cell_values(pts, w, cell_km, origin, shape)


def chamfer_km(a_xy, b_xy):
    a, b = np.asarray(a_xy), np.asarray(b_xy)
    if not len(a) or not len(b):
        return float('inf')
    da, _ = cKDTree(b).query(a)
    db, _ = cKDTree(a).query(b)
    return float((da.mean() + db.mean()) / 2)


def spearman(a, b, mask=None):
    a, b = np.ravel(a), np.ravel(b)
    if mask is not None:
        a, b = a[np.ravel(mask)], b[np.ravel(mask)]
    if np.all(a == a[0]) or np.all(b == b[0]):
        return 0.0
    return float(spearmanr(a, b).statistic)


# --------------------------------------------------------------------------
# The mapped data itself (OpenStreetMap mode)
# --------------------------------------------------------------------------

MAP_RASTER_DEG = 0.02


def _inside_exact(lon, lat, multipolygon):
    """Even-odd ray casting of points against a GeoJSON MultiPolygon's outer
    rings, vectorised over the points."""
    px, py = np.asarray(lon, dtype=float), np.asarray(lat, dtype=float)
    out = np.zeros(px.shape, dtype=bool)
    for poly in multipolygon:
        ring = np.asarray(poly[0], dtype=float)
        hit = np.zeros(px.shape, dtype=bool)
        for (a, b), (c, d) in zip(ring[:-1], ring[1:], strict=True):
            crosses = (b > py) != (d > py)
            with np.errstate(divide='ignore', invalid='ignore'):
                xi = (c - a) * (py - b) / (d - b) + a
            hit ^= crosses & (px < xi)
        out |= hit
    return out


def _region_raster(multipolygon, step=MAP_RASTER_DEG):
    """Inside test for a GeoJSON MultiPolygon, rasterised at `step` degrees
    (about 2 km): a point is inside when its cell centre is. Returns a
    function of (lon, lat) arrays. Used for line vertices, which are many;
    plants and substations take the exact test."""
    rings = [np.asarray(poly[0], dtype=float) for poly in multipolygon]
    allpts = np.vstack(rings)
    lon0, lat0 = allpts.min(axis=0) - step
    nx = int(math.ceil((allpts[:, 0].max() + step - lon0) / step))
    ny = int(math.ceil((allpts[:, 1].max() + step - lat0) / step))
    cx = lon0 + (np.arange(nx) + 0.5) * step
    cy = lat0 + (np.arange(ny) + 0.5) * step
    gx, gy = np.meshgrid(cx, cy, indexing='ij')
    px, py = gx.ravel(), gy.ravel()
    mask = np.zeros(px.shape, dtype=bool)
    for ring in rings:
        hit = np.zeros(px.shape, dtype=bool)
        x1, y1 = ring[:-1, 0], ring[:-1, 1]
        x2, y2 = ring[1:, 0], ring[1:, 1]
        for a, b, c, d in zip(x1, y1, x2, y2, strict=True):
            crosses = (b > py) != (d > py)
            with np.errstate(divide='ignore', invalid='ignore'):
                xi = (c - a) * (py - b) / (d - b) + a
            hit ^= crosses & (px < xi)
        mask |= hit
    mask = mask.reshape(nx, ny)

    def inside(lon, lat):
        i = np.floor((np.asarray(lon, dtype=float) - lon0) / step).astype(int)
        j = np.floor((np.asarray(lat, dtype=float) - lat0) / step).astype(int)
        ok = (i >= 0) & (i < nx) & (j >= 0) & (j < ny)
        out = np.zeros(i.shape, dtype=bool)
        out[ok] = mask[i[ok], j[ok]]
        return out

    return inside


def _osm_kvs(tag):
    out = set()
    for part in re.split(r'[;,/ ]+', tag or ''):
        try:
            v = float(part) / 1000.0
        except ValueError:
            continue
        if math.isfinite(v) and v > 0:
            out.add(v)
    return sorted(out, reverse=True)


def _osm_mw(value):
    s = (value or '').strip().lower().replace(',', '')
    m = re.fullmatch(r'(?:(\d+)\s*x\s*)?([0-9.]+(?:e[+-]?\d+)?)\s*(gw|mw|kw|w|megawatt|mva)?', s)
    if not m:
        return None
    unit = {'gw': 1000.0, 'mw': 1.0, 'megawatt': 1.0, 'mva': 1.0, 'kw': 1e-3, 'w': 1e-6}[m.group(3) or 'w']
    return int(m.group(1) or 1) * float(m.group(2)) * unit


def _osm_fuel(source):
    s = (source or '').split(';')[0].strip().lower()
    for fuel, names in (('gas', ('gas',)), ('oil', ('oil', 'diesel')), ('coal', ('coal', 'lignite')),
                        ('nuclear', ('nuclear',)), ('hydro', ('hydro', 'water')), ('wind', ('wind',)),
                        ('solar', ('solar', 'photovoltaic')), ('storage', ('battery', 'storage'))):
        if any(n in s for n in names):
            return fuel
    return 'other'


def _osm_point(el):
    if 'lat' in el and 'lon' in el:
        return el['lon'], el['lat']
    c = el.get('center')
    if c:
        return c['lon'], c['lat']
    b = el.get('bounds')
    if b:
        return (b['minlon'] + b['maxlon']) / 2, (b['minlat'] + b['maxlat']) / 2
    return None


def measure_map(doc, multipolygon, floor_kv=56.0):
    """What an Overpass document itself holds inside a region, measured as
    the scorecard measures a grid, with no engine involved: nameplate and fuel
    mix of the power=plant features that tag their output (untagged plants
    are counted, not estimated); every substation (an upper bound on sites);
    the voltage levels substations tag at `floor_kv` and above, as bus shares
    per class; and circuit-km per class of the lines that tag a voltage, from
    their circuits (else cables / 3, else 1) over the length of their segments
    inside the region. Those segments are kept (`_segments`: lon, lat of both
    ends and circuits) for the gridded line density, `map_route_samples`."""
    inside = _region_raster(multipolygon)
    cap = defaultdict(float)
    untagged_plants = 0
    substations = 0
    levels = Counter()
    ckm = defaultdict(float)
    segments = []
    pts = {'plant': [], 'substation': []}
    feats = {'plant': [], 'substation': []}
    for el in doc.get('elements', []):
        tags = el.get('tags') or {}
        power = tags.get('power')
        if power in ('plant', 'substation'):
            p = _osm_point(el)
            if p:
                pts[power].append(p)
                feats[power].append(tags)
        elif power in ('line', 'cable'):
            kvs = [k for k in _osm_kvs(tags.get('voltage')) if k >= floor_kv]
            geom = np.array([(g['lon'], g['lat']) for g in el.get('geometry') or [] if g], dtype=float)
            if not kvs or len(geom) < 2:
                continue
            try:
                circuits = int(tags.get('circuits'))
            except (TypeError, ValueError):
                try:
                    circuits = round(float(tags.get('cables')) / 3)
                except (TypeError, ValueError):
                    circuits = 1
            circuits = min(max(circuits, 1), 12)
            ins = inside(geom[:, 0], geom[:, 1])
            seg = ins[:-1] & ins[1:]
            km = haversine_km(geom[:-1, 1], geom[:-1, 0], geom[1:, 1], geom[1:, 0])
            ckm[str(int(kv_class([kvs[0]])[0]))] += float(np.sum(km[seg])) * circuits
            if seg.any():
                ends = np.column_stack([geom[:-1][seg], geom[1:][seg], np.full(int(seg.sum()), circuits)])
                segments.append(ends)
    for kind in ('plant', 'substation'):
        if not pts[kind]:
            continue
        xy = np.array(pts[kind], dtype=float)
        for ok, tags in zip(_inside_exact(xy[:, 0], xy[:, 1], multipolygon), feats[kind], strict=True):
            if not ok:
                continue
            if kind == 'plant':
                mw = _osm_mw(tags.get('plant:output:electricity'))
                if mw is None:
                    untagged_plants += 1
                else:
                    cap[_osm_fuel(tags.get('plant:source'))] += mw
            else:
                substations += 1
                for k in _osm_kvs(tags.get('voltage')):
                    if k >= floor_kv:
                        levels[float(kv_class([k])[0])] += 1
    total_levels = max(sum(levels.values()), 1)
    return {
        'nameplate_mw': float(sum(cap.values())),
        'fuel_mix': _shares(cap, FUELS),
        'untagged_plants': untagged_plants,
        'sites': substations,
        'bus_share': {str(int(c)): levels.get(c, 0) / total_levels for c in CLASSES_KV},
        'circuit_km': {str(int(c)): ckm.get(str(int(c)), 0.0) for c in CLASSES_KV},
        '_segments': np.vstack(segments) if segments else np.zeros((0, 5)),
    }


def map_route_samples(mapm, lat0, lon0, step_km=1.0):
    """Points every `step_km` along the map's line segments, weighted by
    length times circuits (a generated grid's samples count each circuit's
    branch), in the frame of `project(…, lat0, lon0)`."""
    seg = mapm['_segments']
    if len(seg) == 0:
        return np.zeros((0, 2)), np.zeros(0)
    a = project(seg[:, 1], seg[:, 0], lat0, lon0)
    b = project(seg[:, 3], seg[:, 2], lat0, lon0)
    pts, w = [], []
    for p, q, c in zip(a, b, seg[:, 4], strict=True):
        length = float(np.linalg.norm(q - p))
        m = max(1, int(math.ceil(length / step_km)))
        t = (np.arange(m) + 0.5) / m
        pts.append(p[None, :] + t[:, None] * (q - p)[None, :])
        w.append(np.full(m, length / m * c))
    return np.vstack(pts), np.concatenate(w)
