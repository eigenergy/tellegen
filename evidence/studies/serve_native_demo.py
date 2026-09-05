#!/usr/bin/env python3
"""Serve the built application with a synthetic case and its exact native result."""
import argparse
import json
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--web-build', type=Path, default=Path('apps/web/build'))
    parser.add_argument('--input', type=Path, default=Path('evidence/studies/native-webmcp/input.pio.json'))
    parser.add_argument('--solution', type=Path, default=Path('evidence/studies/native-webmcp/initial-solution.json'))
    parser.add_argument('--port', type=int, default=4186)
    args = parser.parse_args()
    module = json.loads(args.input.read_text())
    net = module['value']['data']
    solved = json.loads(args.solution.read_text())
    coords = {1: (-81.1, 34.0), 2: (-81.0, 34.1), 3: (-80.9, 34.0)}
    if {b['id'] for b in net['buses']} != coords.keys():
        raise ValueError('This demonstration requires the synthetic three-bus case')
    network = dict(
        id='study-demo', name='Synthetic corridor study', base_mva=net['base_mva'],
        synthetic_coords=True,
        buses=[dict(id=b['id'], uid=b['uid'], lon=coords[b['id']][0], lat=coords[b['id']][1],
                    demand_mw=sum(l['p'] for l in net['loads'] if l['bus'] == b['id']),
                    gen_mw=sum(g['pmax'] for g in net['generators'] if g['bus'] == b['id']))
               for b in net['buses']],
        branches=[dict(id=i + 1, uid=b['uid'], **{'from': b['from'], 'to': b['to']},
                       rate_mw=b['rate_a'], status=int(b['in_service']),
                       path=[coords[b['from']], coords[b['to']]])
                  for i, b in enumerate(net['branches'])],
    )
    solution = dict(
        objective=solved['objective'], prices=solved['lmp'], va=solved['va'], w=[],
        flows=[dict(branch=f['branch'], mw=f['pf'], loading=f['loading']) for f in solved['flows']],
        dispatch=[dict(gen=g['gen'], mw=g['pg']) for g in solved['dispatch']],
    )
    routes = {
        '/api/compute': {'enabled': False},
        '/api/cases': [dict(id='study-demo', name='Synthetic corridor study', n_bus=3, n_branch=3, n_gen=2)],
        '/api/cases/study-demo/network': network,
        '/api/cases/study-demo/solution': solution,
        '/api/cases/study-demo/case': module,
    }

    class Handler(SimpleHTTPRequestHandler):
        def do_GET(self):
            if self.path not in routes:
                return super().do_GET()
            body = json.dumps(routes[self.path]).encode()
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    handler = partial(Handler, directory=str(args.web_build.resolve()))
    ThreadingHTTPServer(('127.0.0.1', args.port), handler).serve_forever()


if __name__ == '__main__':
    main()
