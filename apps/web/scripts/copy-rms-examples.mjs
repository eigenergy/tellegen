// Copy the WECC 240 dynamics demo into static/examples/wecc240 from its one
// source of truth, crates/tellegen/tests/data/rms_examples/wecc240, and
// write case-validation.raw: the same raw file with the loads of the
// PowerSimulationsDynamics.jl validation study (psid_reference.json), each
// load's constant power set to the study's value and its current and
// admittance parts to zero, exactly as crates/tellegen/tests/rms_psid_validation.rs
// builds it. Run by predev and prebuild; the output is git-ignored.
import { copyFileSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const source = join(here, '../../../crates/tellegen/tests/data/rms_examples/wecc240');
const target = join(here, '../static/examples/wecc240');
mkdirSync(target, { recursive: true });
for (const name of readdirSync(source)) {
	if (name === 'psid_reference.json') continue;
	copyFileSync(join(source, name), join(target, name));
}

const raw = readFileSync(join(source, 'case.raw'), 'utf8');
const loads = JSON.parse(readFileSync(join(source, 'psid_reference.json'), 'utf8')).wecc240
	.loads_mw_mvar;
const byBus = new Map();
for (const [bus, p, q] of loads) {
	if (!byBus.has(bus)) byBus.set(bus, []);
	byBus.get(bus).push([p, q]);
}
const lines = raw.split('\n');
const start = lines.findIndex((l) => l.includes('Begin Load data')) + 1;
const end = lines.findIndex((l) => l.includes('End of Load data'));
for (let i = start; i < end; i++) {
	const f = lines[i].split(',');
	const bus = Number(f[0].trim());
	const next = byBus.get(bus)?.shift();
	if (!next) throw new Error(`no validation load for bus ${bus}`);
	f[5] = next[0].toFixed(10).padStart(18);
	f[6] = next[1].toFixed(10).padStart(18);
	for (let k = 7; k <= 10; k++) f[k] = '     0.000';
	lines[i] = f.join(',');
}
for (const [bus, rest] of byBus)
	if (rest.length) throw new Error(`unused validation load at bus ${bus}`);
writeFileSync(join(target, 'case-validation.raw'), lines.join('\n'));
