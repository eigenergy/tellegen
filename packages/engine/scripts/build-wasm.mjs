import { spawnSync } from 'node:child_process';

// Optional downstream compatibility builds must select these explicitly. The
// normal published package continues to use its locked, released dependency.
const optional = (process.env.TELLEGEN_POWERIO_FEATURES ?? '').split(',').filter(Boolean);
for (const feature of optional) {
  if (!['powerio-012', 'powerio-collections'].includes(feature))
    throw new Error(`Unsupported PowerIO build feature: ${feature}`);
}
const built = spawnSync('wasm-pack', [
  'build', '../../crates/tellegen-wasm', '--target', 'web',
  '--out-dir', '../../packages/engine/src/wasm-pkg', '--out-name', 'tellegen',
  '--', '--features', ['conic', 'synth', ...optional].join(',')
], { stdio: 'inherit' });
if (built.error) throw built.error;
if (built.status !== 0) process.exit(built.status ?? 1);
await import('./copy-synth-places.mjs');
