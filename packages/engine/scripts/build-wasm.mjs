import { spawnSync } from 'node:child_process';

// Compatibility checks can add feature aliases. Collections ship in the normal
// build using the released PowerIO dependency.
const optional = (process.env.TELLEGEN_POWERIO_FEATURES ?? '').split(',').filter(Boolean);
for (const feature of optional) {
  if (!['powerio-012', 'powerio-collections'].includes(feature))
    throw new Error(`Unsupported PowerIO build feature: ${feature}`);
}
const built = spawnSync('wasm-pack', [
  'build', '../../crates/tellegen-wasm', '--target', 'web',
  '--out-dir', '../../packages/engine/src/wasm-pkg', '--out-name', 'tellegen',
  // `rms-experimental` ships in the binary; the app gates its tools behind
  // the experimental switch.
  '--', '--features', ['conic', 'synth', 'rms', 'rms-experimental', 'powerio-collections', ...optional].join(',')
], { stdio: 'inherit' });
if (built.error) throw built.error;
if (built.status !== 0) process.exit(built.status ?? 1);
await import('./copy-synth-places.mjs');
