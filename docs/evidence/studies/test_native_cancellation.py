"""Verify that a real native SIGTERM preserves completed planning trials."""
import json
import os
from pathlib import Path
import selectors
import signal
import subprocess
import tempfile
import time
import unittest

CASE = '''function mpc = cancellation_case
mpc.version = '2';
mpc.baseMVA = 100;
mpc.bus = [1 3 0 0 0 0 1 1 0 230 1 1.1 0.9;
2 2 2500 0 0 0 1 1 0 230 1 1.1 0.9;];
mpc.gen = [1 0 0 3000 -3000 1 100 1 5000 0;
2 2500 0 3000 -3000 1 100 1 5000 0;];
mpc.branch = [1 2 0.01 0.1 0 35 35 35 0 0 1 -360 360;];
mpc.gencost = [2 0 0 3 0.01 1 0; 2 0 0 3 0.1 10 0;];
'''


@unittest.skipUnless(os.name == 'posix' and os.getenv('TELLEGEN_STUDY_CLI') and os.getenv('POWERIO_CLI'), 'requires native CLI paths and POSIX signals')
class NativeCancellation(unittest.TestCase):
    def test_completed_trials_survive_sigterm(self):
        cli, powerio = os.environ['TELLEGEN_STUDY_CLI'], os.environ['POWERIO_CLI']
        with tempfile.TemporaryDirectory(prefix='study-signal-') as directory:
            root = Path(directory)
            source, ir, path = root / 'case.m', root / 'case.pio.json', root / 'study.json'
            source.write_text(CASE)
            subprocess.run([powerio, 'serialize', str(source), '-o', str(ir)], check=True, capture_output=True)
            create = {'id': 'signal-test', 'title': 'Native cancellation', 'input': ir.read_text(),
                      'formulation': 'dcopf', 'request': 'Lower the load bus price', 'interpretation': 'Bounded capacity upgrades',
                      'objective': {'kind': 'weighted_observable', 'operand': {'Price': 'Active'}, 'weights': [{'element': 2, 'weight': 1}]},
                      'decisions': {'variables': [{'id': 'line', 'element': 1, 'intervention': 'branch_rating', 'lower': 0, 'upper': 1000, 'increment': 1, 'budget_weight': 1}], 'total_budget': 1000, 'max_changed_elements': 1}}
            subprocess.run([cli, 'study', 'create', str(path)], input=json.dumps(create), text=True, check=True, capture_output=True)
            initial = json.loads(path.read_text())['document']
            request = {'expected_revision': initial['revision'], 'operation': {'kind': 'propose', 'state': initial['applied_state'], 'goal': initial['active_goal'], 'rationale': 'Test cancellation after a completed exact trial', 'options': {'max_solves': 256, 'max_iterations': 256, 'beam_width': 1, 'min_improvement': 1e-9}}}
            child = subprocess.Popen([cli, 'study', 'run', str(path), '--progress'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            try:
                child.stdin.write(json.dumps(request).encode())
                child.stdin.close()
                child.stdin = None
                deadline = time.monotonic() + 30
                buffered = b''
                with selectors.DefaultSelector() as selector:
                    selector.register(child.stderr, selectors.EVENT_READ)
                    while True:
                        events = selector.select(max(0, deadline - time.monotonic()))
                        self.assertTrue(events, 'Native process did not reach a completed-trial checkpoint')
                        chunk = os.read(child.stderr.fileno(), 4096)
                        self.assertTrue(chunk, 'Native process exited before cancellation')
                        buffered += chunk
                        lines = buffered.split(b'\n')
                        buffered = lines.pop()
                        if any(json.loads(line).get('index', 0) >= 4 for line in lines if line):
                            child.send_signal(signal.SIGTERM)
                            break
                output, error = child.communicate(timeout=30)
                self.assertEqual(child.returncode, 0, error.decode())
                result = json.loads(output)
                bundle = json.loads(path.read_text())
                record = bundle['document']['experiments'][result['experiment']]
                self.assertEqual(record['termination'], 'cancelled')
                self.assertGreaterEqual(len(record['trials']), 1)
                self.assertEqual(record['solve_count'], len(record['trials']) + 1)
                self.assertEqual(bundle['document']['applied_state'], initial['applied_state'])
                self.assertIsNotNone(bundle['document']['recommended_state'])
            finally:
                if child.poll() is None:
                    child.kill()
                    child.communicate()


if __name__ == '__main__':
    unittest.main()
