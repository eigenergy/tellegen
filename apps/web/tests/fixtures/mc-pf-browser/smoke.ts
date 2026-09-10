import { solveMcBmopf, solveMcModule } from '@tellegen/engine';

type Complex = { re: number; im: number };
type Terminal = { bus: string; terminal: string; voltage: Complex };

const resultNode = document.querySelector<HTMLElement>('#result');

async function main() {
	const [raw, moduleJson, oracle] = await Promise.all([
		fetch('/mc-pf-three-phase.bmopf.json').then((response) => response.text()),
		fetch('/mc-pf-three-phase.pio.json').then((response) => response.text()),
		fetch('/mc-pf-three-phase.oracle.json').then((response) => response.json())
	]);
	// These frozen OpenDSS references use the original load equations, as in
	// the native oracle tests. The Study tests exercise the default envelope.
	const options = { tolerance: 1e-8, max_iterations: 100, voltage_envelope: false };
	const rawResult = await solveMcBmopf(raw, options);
	const typedResult = await solveMcModule(moduleJson, options);
	const oracleTerminals = oracle.terminals as Terminal[];
	const actual = new Map(
		rawResult.terminals.map((terminal) => [`${terminal.bus}:${terminal.terminal}`, terminal])
	);
	const typed = new Map(
		typedResult.terminals.map((terminal) => [`${terminal.bus}:${terminal.terminal}`, terminal])
	);
	if (actual.size !== oracleTerminals.length || typed.size !== oracleTerminals.length) {
		throw new Error(
			`terminal coverage mismatch raw=${actual.size} typed=${typed.size} oracle=${oracleTerminals.length}`
		);
	}
	let maxOracleError = 0;
	let maxTypedError = 0;
	for (const expected of oracleTerminals) {
		const key = `${expected.bus}:${expected.terminal}`;
		const terminal = actual.get(key);
		const typedTerminal = typed.get(key);
		if (!terminal || !typedTerminal) throw new Error(`missing terminal ${key}`);
		if (
			!Number.isFinite(terminal.voltage.re) ||
			!Number.isFinite(terminal.voltage.im) ||
			!Number.isFinite(typedTerminal.voltage.re) ||
			!Number.isFinite(typedTerminal.voltage.im)
		) {
			throw new Error(`non-finite terminal ${key}`);
		}
		maxOracleError = Math.max(maxOracleError, distance(terminal.voltage, expected.voltage));
		maxTypedError = Math.max(maxTypedError, distance(terminal.voltage, typedTerminal.voltage));
	}
	const invalid = JSON.parse(raw) as { load: Record<string, { model?: string }> };
	const firstLoad = Object.values(invalid.load)[0];
	firstLoad.model = 'unsupported_model';
	let invalidRejected = false;
	try {
		await solveMcBmopf(JSON.stringify(invalid), options);
	} catch {
		invalidRejected = true;
	}
	if (!invalidRejected) throw new Error('unsupported load model was accepted');
	const payload = {
		raw: rawResult,
		typed: typedResult,
		max_oracle_voltage_error: maxOracleError,
		max_typed_voltage_error: maxTypedError,
		oracle_terminal_count: oracleTerminals.length,
		raw_terminal_count: actual.size,
		typed_terminal_count: typed.size,
		invalid_rejected: invalidRejected
	};
	if (resultNode) resultNode.textContent = JSON.stringify(payload);
}

function distance(left: Complex, right: Complex): number {
	return Math.hypot(left.re - right.re, left.im - right.im);
}

void main().catch((error: unknown) => {
	if (resultNode) resultNode.textContent = JSON.stringify({ error: String(error) });
});
