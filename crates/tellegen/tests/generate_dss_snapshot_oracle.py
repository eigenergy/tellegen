#!/usr/bin/env python3
"""Synthetic fixed snapshots, generated with the CI-pinned OpenDSS backend.

No external feeder data. Exercise actual finite sources, load voltage bands,
ZIP dropout, an explicit fourth wire, and a finite-leakage transformer.
"""
from pathlib import Path

import opendssdirect as dss
from generate_mc_load_oracle import stable_float, write_report


def reference(name, commands, tolerance=1e-12):
    ctx = dss.NewContext()
    ctx.Basic.AllowDOScmd(False)
    ctx.Basic.AllowEditor(False)
    for command in commands:
        ctx.Text.Command(command)
    ctx.Text.Command(f'set mode=snapshot controlmode=off tolerance={tolerance} maxiterations=1000')
    ctx.Solution.Solve()
    assert ctx.Solution.Converged(), name
    nodes = ctx.Circuit.AllNodeNames()
    volts = ctx.Circuit.AllBusVolts()
    voltages = {node: [stable_float(r), stable_float(i)] for node, r, i in zip(nodes, volts[::2], volts[1::2])}
    currents = {}
    for load in ctx.Loads.AllNames():
        ctx.Circuit.SetActiveElement('load.' + load)
        values = ctx.CktElement.Currents()
        currents[load] = {'terminal': str(ctx.CktElement.NodeOrder()[0]), 'value': [stable_float(values[0]), stable_float(values[1])]}
    transformers = {}
    for transformer in ctx.Transformers.AllNames():
        if transformer == 'NONE':
            continue
        ctx.Circuit.SetActiveElement('transformer.' + transformer)
        values = ctx.CktElement.Currents()
        count = ctx.CktElement.NumConductors()
        buses = ctx.CktElement.BusNames()
        ports = {}
        for index, node in enumerate(ctx.CktElement.NodeOrder()):
            if node == 0:
                continue
            label = buses[index // count].split('.')[0] + '.' + str(node)
            prior = ports.setdefault(label, [0., 0.])
            prior[0] += values[2 * index]
            prior[1] += values[2 * index + 1]
        transformers[transformer] = dict(ports={k: [stable_float(v) for v in values] for k, values in ports.items()}, losses=[stable_float(v) for v in ctx.CktElement.Losses()])
    return dict(name=name, tolerance=tolerance, source='\n'.join(commands) + '\n', voltages=voltages, currents=currents, transformers=transformers)


def generate():
    cases = []
    for model, extra in [(1, ''), (2, ''), (3, ''), (4, 'cvrwatts=.7 cvrvars=2.3'), (5, ''),
                         (8, 'zipv=[.2,.3,.5,.5,.1,.4,.7]')]:
        for pu in [.2, .6, .7, .9, 1, 1.2]:
            commands = ['clear', 'set defaultbasefrequency=50',
                        f'new circuit.qc phases=1 bus1=src.1.0 basekv=.24 pu={pu} angle=23 frequency=50',
                        'new line.l phases=1 bus1=src.1 bus2=lb.1 rmatrix=[.07] xmatrix=[.02] cmatrix=[0] length=1 units=m',
                        f'new load.ld phases=1 bus1=lb.1.0 conn=wye kv=.24 kw=2.4 kvar=.8 model={model} vlowpu=.5 vminpu=.85 vmaxpu=1.15 {extra}']
            cases.append(reference(f'model{model}_{pu}', commands))
    commands = ['clear', 'set defaultbasefrequency=50',
                'new circuit.wires phases=3 bus1=src.1.2.3 basekv=.4 frequency=50',
                'new line.l phases=4 bus1=src.1.2.3.0 bus2=lb.1.2.3.4 length=1 units=m rmatrix=[.05 | .01 .05 | .01 .01 .05 | .01 .01 .01 .08] xmatrix=[.02 | .005 .02 | .005 .005 .02 | .005 .005 .005 .03] cmatrix=[0 | 0 0 | 0 0 0 | 0 0 0 0]',
                'new load.a phases=1 bus1=lb.1.4 kv=.23094 kw=3 kvar=1 model=1',
                'new load.b phases=1 bus1=lb.2.4 kv=.23094 kw=2 kvar=.5 model=5',
                'new load.c phases=1 bus1=lb.3.4 kv=.23094 kw=1 kvar=.2 model=4 cvrwatts=.8 cvrvars=2']
    cases.append(reference('four_wire', commands))
    commands = ['clear', 'set defaultbasefrequency=50',
                'new circuit.tx phases=3 bus1=src.1.2.3 basekv=11 frequency=50',
                'new transformer.tx phases=3 windings=2 buses=[src.1.2.3 lv.1.2.3.0] conns=[delta wye] kvs=[11 .4] kvas=[100 100] %rs=[.5 .5] xhl=4 %noloadloss=.2 %imag=1',
                'new load.ld phases=3 bus1=lv.1.2.3.0 conn=wye kv=.4 kw=30 kvar=10 model=1']
    cases.append(reference('delta_wye', commands))
    for connections in ['wye wye delta', 'delta wye wye']:
        for ppm in [None, 0, 50]:
            written = '' if ppm is None else f' ppm_antifloat={ppm}'
            commands = ['clear', 'set defaultbasefrequency=50',
                        'new circuit.three phases=3 bus1=src.1.2.3 basekv=11 frequency=50',
                        f'new transformer.tx phases=3 windings=3 buses=[src.1.2.3.0 lv.1.2.3.0 third.1.2.3.0] conns=[{connections}] kvs=[11 .4 4.16] kvas=[1000 1000 1000] taps=[1.025 .975 1.01] %rs=[.2 .3 .4] xhl=4 xht=5 xlt=6{written}',
                        'new load.lv phases=3 bus1=lv.1.2.3.0 conn=wye kv=.4 kw=30 kvar=10 model=1',
                        'new load.third phases=3 bus1=third.1.2.3.0 conn=wye kv=4.16 kw=20 kvar=5 model=1']
            cases.append(reference(f'three_{connections.replace(" ", "_")}_{ppm}', commands, tolerance=1e-10))
    # The delta secondary and its delta load have no path to ground, so only
    # the transformer's ppm_antifloat admittance fixes the winding's voltage to ground.
    for ppm in [None, 0, 50]:
        written = '' if ppm is None else f' ppm_antifloat={ppm}'
        commands = ['clear', 'set defaultbasefrequency=50',
                    'new circuit.floating phases=3 bus1=src.1.2.3 basekv=11 frequency=50',
                    f'new transformer.tx phases=3 windings=2 buses=[src.1.2.3.0 lv.1.2.3] conns=[wye delta] kvs=[11 .4] kvas=[100 100] %rs=[.5 .5] xhl=4{written}',
                    'new load.ld phases=3 bus1=lv.1.2.3 conn=delta kv=.4 kw=30 kvar=10 model=1']
        cases.append(reference(f'floating_delta_{ppm}', commands, tolerance=1e-10))
    for ppm in [None, 0, 50]:
        written = '' if ppm is None else f' ppm_antifloat={ppm}'
        commands = ['clear', 'set defaultbasefrequency=50',
                    'new circuit.reg phases=1 bus1=src.1.0 basekv=2.4 frequency=50',
                    f'new transformer.tx phases=1 windings=2 buses=[src.1.0 lv.1.0] conns=[wye wye] kvs=[2.4 2.4] kvas=[100 100] taps=[1 1.025] %rs=[.2 .2] xhl=1{written}',
                    'new load.ld phases=1 bus1=lv.1.0 kv=2.4 kw=10 kvar=2 model=1',
                    'new regcontrol.r transformer=tx winding=2 vreg=120 ptratio=20']
        case = reference(f'fixed_regulator_{ppm}', commands)
        case['fixed_controls'] = True
        cases.append(case)
    return {'backend': 'OpenDSSDirect.py 0.9.4; DSS-Python 0.15.7; DSS C-API 0.14.5', 'cases': cases}


if __name__ == '__main__':
    write_report(Path(__file__).with_name('data') / 'dss_snapshots.json', generate())
