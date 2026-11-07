#!/usr/bin/env python3
"""Synthetic fixed snapshots, generated with the CI-pinned OpenDSS backend.

No external feeder data. Exercise actual finite sources, load voltage bands,
ZIP dropout, an explicit fourth wire, and a finite-leakage transformer.
"""
import json
from pathlib import Path
import opendssdirect as dss
from generate_mc_load_oracle import stable_float, write_report


def reference(name, commands):
    ctx = dss.NewContext()
    ctx.Basic.AllowDOScmd(False)
    ctx.Basic.AllowEditor(False)
    for command in commands:
        ctx.Text.Command(command)
    ctx.Text.Command('set mode=snapshot controlmode=off tolerance=1e-12 maxiterations=1000')
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
    return dict(name=name, source='\n'.join(commands) + '\n', voltages=voltages, currents=currents)


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
    return {'backend': 'OpenDSSDirect.py 0.9.4; DSS-Python 0.15.7; DSS C-API 0.14.5', 'cases': cases}


if __name__ == '__main__':
    write_report(Path(__file__).with_name('data') / 'dss_snapshots.json', generate())
