"""Connected on/off perspective formulation of flat-start LACOTS."""

import itertools
import json
import math

from . import _tellegen


def _angle_boxes(data):
    """Derive valid on-state bounds from terminal limits and nodal balance."""
    buses = data["buses"]
    generators = data["generators"]
    cut = sum(
        abs(b["pd"]) + abs(b["gs"]) * max(abs(1 + 2 * h) for h in b["h"]) for b in buses
    )
    reciprocal = all(g["p"] and None not in g["p"] for g in generators)
    if reciprocal:
        cut += sum(max(map(abs, g["p"])) for g in generators)
    boxes = []
    for line in data["branches"]:
        f, t = line["buses"]
        cap = math.inf
        slopes = []
        for side, (p, q) in enumerate(line["ends"]):
            slopes.append(p[3])
            angle_slope = abs(complex(p[3], q[3]))
            corners = [
                complex(p[0] + p[1] * hf + p[2] * ht, q[0] + q[1] * hf + q[2] * ht)
                for hf, ht in itertools.product(buses[f]["h"], buses[t]["h"])
            ]
            cut += max(abs(s.real) for s in corners)
            limit = line["rating"]
            current = line["current"][side]
            if current is not None:
                thermal = current * (1 + max(buses[(f, t)[side]]["h"]))
                limit = thermal if limit is None else min(limit, thermal)
            if limit is not None and angle_slope:
                cap = min(cap, (limit + max(map(abs, corners))) / angle_slope)
        reciprocal = reciprocal and slopes[0] > 0 and slopes[0] == -slopes[1]
        boxes.append((cap, slopes[0]))
    intervals = []
    for line, (terminal_bound, slope) in zip(data["branches"], boxes, strict=True):
        # Across a high-angle cut, every angular P flow has the same sign.
        # Internal angular terms cancel for reciprocal slopes. The sum of
        # absolute injections and voltage-only terms therefore bounds each flow.
        cap = min(terminal_bound, cut / slope) if reciprocal else terminal_bound
        lo, hi = line["angle"] or (None, None)
        lo = max(-cap, lo) if lo is not None else -cap
        hi = min(cap, hi) if hi is not None else cap
        if not math.isfinite(hi - lo) or lo > hi:
            raise ValueError(
                f"no valid finite on-state angle interval for {line['id']}"
            )
        intervals.append((lo, hi))
    return intervals


def _solve(data, intervals, bridges, discrete, seconds):
    from pyscipopt import Model, quicksum

    m = Model("flat-start LACOTS")
    m.hideOutput()
    m.setRealParam("limits/time", seconds)
    m.setRealParam("limits/gap", 0.0)
    m.setRealParam("numerics/feastol", 1e-8)
    m.setIntParam("parallel/maxnthreads", 1)
    buses = data["buses"]
    n = len(buses)
    root = data["root"]
    weights = [max(abs(a), abs(b)) for a, b in intervals]
    theta_bound = sum(sorted(weights, reverse=True)[: n - 1])
    inf = m.infinity()

    def var(name, lo, hi):
        return m.addVar(
            name, lb=-inf if lo is None else lo, ub=inf if hi is None else hi
        )

    theta = [
        var(
            f"theta_{i}",
            0 if i == root else -theta_bound,
            0 if i == root else theta_bound,
        )
        for i in range(n)
    ]
    h = [var(f"dv_{i}", *bus["h"]) for i, bus in enumerate(buses)]
    pnet, qnet, commodity = ([[] for _ in buses] for _ in range(3))
    objective = []
    for i, g in enumerate(data["generators"]):
        pg = var(f"pg_{i}", *(g["p"] or [None, None]))
        qg = var(f"qg_{i}", *(g["q"] or [None, None]))
        pnet[g["bus"]].append(pg)
        qnet[g["bus"]].append(qg)
        objective.append(g["constant"] / 1000)
        ep = var(f"cost_{i}", None, None)
        if g["segments"] is not None:
            for a, b in g["segments"]:
                m.addCons(ep >= (a * pg + b) / 1000)
        else:
            m.addCons(ep >= (g["quadratic"] * pg * pg + g["linear"] * pg) / 1000)
        objective.append(ep)

    def on_state(name, value, lo, hi, z):
        u = var(name, min(0, lo), max(0, hi))
        m.addCons(u >= lo * z)
        m.addCons(u <= hi * z)
        m.addCons(value - u >= lo * (1 - z))
        m.addCons(value - u <= hi * (1 - z))
        return u

    states = []
    for k, (line, interval) in enumerate(zip(data["branches"], intervals, strict=True)):
        f, t = line["buses"]
        fixed = k in bridges or not line["switchable"]
        z = m.addVar(
            f"z_{k}", vtype="B" if discrete and not fixed else "C", lb=int(fixed), ub=1
        )
        states.append(z)
        objective.append(line["cost"] * (1 - z) / 1000)
        flow = var(f"connect_{k}", 1 - n, n - 1)
        m.addCons(flow <= (n - 1) * z)
        m.addCons(flow >= -(n - 1) * z)
        commodity[f].append(flow)
        commodity[t].append(-flow)
        lo, hi = interval
        d = theta[f] - theta[t]
        if fixed:
            df, hf, ht = d, h[f], h[t]
            m.addCons(d >= lo)
            m.addCons(d <= hi)
        else:
            # A connected off-state admits a simple path of at most n-1 other lines.
            off = sum(sorted(weights[:k] + weights[k + 1 :], reverse=True)[: n - 1])
            df = var(f"on_angle_{k}", min(0, lo), max(0, hi))
            m.addCons(df >= lo * z)
            m.addCons(df <= hi * z)
            m.addCons(d - df >= -off * (1 - z))
            m.addCons(d - df <= off * (1 - z))
            hf = on_state(f"on_vf_{k}", h[f], *buses[f]["h"], z)
            ht = on_state(f"on_vt_{k}", h[t], *buses[t]["h"], z)
        for side, (pcoef, qcoef) in enumerate(line["ends"]):
            bus = (f, t)[side]
            p = var(f"p_{k}_{side}", None, None)
            q = var(f"q_{k}_{side}", None, None)
            cols = (z, hf, ht, df)
            for v, coefs in ((p, pcoef), (q, qcoef)):
                m.addCons(
                    v == quicksum(a * x for a, x in zip(coefs, cols, strict=True))
                )
            pnet[bus].append(-p)
            qnet[bus].append(-q)
            if line["rating"] is not None:
                m.addCons(p * p + q * q <= (line["rating"] * z) ** 2)
            if line["current"][side] is not None:
                m.addCons(
                    p * p + q * q <= (line["current"][side] * (z + (hf, ht)[side])) ** 2
                )
    for i, bus in enumerate(buses):
        m.addCons(quicksum(commodity[i]) == (n - 1 if i == root else -1))
        m.addCons(quicksum(pnet[i]) == bus["pd"] + bus["gs"] * (1 + 2 * h[i]))
        m.addCons(quicksum(qnet[i]) == bus["qd"] - bus["bs"] * (1 + 2 * h[i]))
    m.addCons(quicksum(states) >= n - 1)
    for cap in (data["max_open"], data["max_switched"]):
        if cap is not None:
            m.addCons(quicksum(states) >= len(states) - cap)
    m.setObjective(quicksum(objective))
    m.optimize()
    lower = m.getDualbound()
    sol = m.getBestSol()
    return {
        "status": str(m.getStatus()),
        "seconds": m.getSolvingTime(),
        "lower_bound": 1000 * lower if abs(lower) < 0.9 * inf else None,
        "objective": 1000 * m.getSolObjVal(sol) if sol is not None else None,
        "closed": [m.getSolVal(sol, z) for z in states] if sol is not None else None,
    }


def solve_lacots_gap(module_json: str, *, time_limit: float = 120.0) -> dict:
    """Compare integer switching with z in [0,1] using SCIP, in cost units.

    Each solve gets time_limit seconds. The interval bounds the absolute
    objective gap even when SCIP stops early. A point percentage requires
    optimal termination of both solves and a positive integer objective.
    """
    try:
        import networkx as nx
        import pyscipopt  # noqa: F401
    except ImportError as exc:
        raise ImportError("Install tellegen[switching] to compute LACOTS gaps") from exc
    if not math.isfinite(time_limit) or time_limit <= 0:
        raise ValueError("time_limit must be finite and positive")
    data = json.loads(_tellegen.lacots_perspective_data(module_json))
    graph = nx.MultiGraph()
    graph.add_nodes_from(range(len(data["buses"])))
    for k, line in enumerate(data["branches"]):
        graph.add_edge(*line["buses"], key=k)
    bridges = {next(iter(graph[f][t])) for f, t in nx.bridges(graph)}
    intervals = _angle_boxes(data)
    relaxed = _solve(data, intervals, bridges, False, time_limit)
    integer = _solve(data, intervals, bridges, True, time_limit)
    lb, ub = integer["lower_bound"], integer["objective"]
    rl, ru = relaxed["lower_bound"], relaxed["objective"]
    interval = [
        max(0.0, lb - ru) if lb is not None and ru is not None else None,
        max(0.0, ub - rl) if ub is not None and rl is not None else None,
    ]
    percent = None
    if (
        integer["status"] == relaxed["status"] == "optimal"
        and ub is not None
        and ub > 0
    ):
        percent = 100 * max(0.0, ub - ru) / ub
    return {
        "branch_ids": [e["id"] for e in data["branches"]],
        "integer": integer,
        "relaxed": relaxed,
        "gap_interval": interval,
        "gap_percent": percent,
    }
