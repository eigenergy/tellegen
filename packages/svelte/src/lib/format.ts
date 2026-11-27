import type { RGBA } from './colors.js';
import type { DisplayMode, SolveBackend } from './state.svelte.js';
import { FORMULATIONS, type Formulation, type ObjectiveKind } from '@tellegen/engine';

// errorText is defined once in wasm.ts (its first consumer) and surfaced here so UI
// code can import it alongside the other formatting helpers.
export { errorText } from '@tellegen/engine';

/** The short menu label for a formulation tag (e.g. `acopf` -> `AC OPF`). */
export function formulationLabel(id: Formulation, objectiveKind?: ObjectiveKind | null): string {
	if (objectiveKind === 'feasibility' && id === 'dcopf') return 'DC feasibility';
	if (objectiveKind === 'feasibility' && id === 'acopf') return 'AC feasibility';
	if (objectiveKind === 'feasibility' && id === 'socwr') return 'AC feasibility (relaxed)';
	return FORMULATIONS.find((f) => f.id === id)?.label ?? id;
}

const BACKEND_LABELS: Record<SolveBackend, string> = {
	'clarabel-wasm': 'Clarabel (WebAssembly)',
	'pounce-wasi': 'POUNCE (WebAssembly)',
	'clarabel-wasm-server-sensitivity': 'Clarabel (WebAssembly), sensitivities on the server',
	'rust-server': 'Server'
};

/** The solver behind a case's last solve, as a reader names it. */
export function solveBackendLabel(backend: SolveBackend): string {
	return BACKEND_LABELS[backend];
}

/** Older saved studies may not name their objective. Keep that distinct from feasibility. */
export function objectiveKind(result: object | null | undefined): ObjectiveKind | undefined {
	if (
		result &&
		'objective_kind' in result &&
		(result.objective_kind === 'cost' || result.objective_kind === 'feasibility')
	)
		return result.objective_kind;
	return undefined;
}

/** A line's name everywhere it is listed: its id and the buses it joins. */
export function lineName(line: { id: number; from: number; to: number }): string {
	return `Line ${line.id} (${line.from}–${line.to})`;
}

export function formulationHint(id: Formulation): string {
	return FORMULATIONS.find((f) => f.id === id)?.hint ?? id;
}

/** A case name and its detail: "IEEE 118 (Region)" -> ["IEEE 118", "Region"]. A trailing
 * file-style identifier ("Texas7k distribution p1uhs0_1247") becomes the detail, spaced. */
export function splitName(name: string): [string, string] {
	const m = name.match(/^(.*?)\s*\((.*)\)$/);
	if (m) return [m[1], m[2]];
	const id = name.match(/^(.*\S)\s+(\S*_\S*)$/);
	return id ? [id[1], id[2].replace(/_/g, ' ')] : [name, ''];
}

export const INFEASIBLE_DISPATCH = 'Infeasible';

/** A line counts as at its limit from 99.9% of its rating, wherever the solver
 * does not flag binding lines itself. */
export const atLimit = (flow: { loading: number }) => flow.loading >= 0.999;

/** A number to six significant figures, without trailing zeros. */
export const sixFigures = (n: number) => Number(n.toPrecision(6)).toString();

/** Hand a file to the viewer. Nothing leaves the machine: a save is a local
 * Blob download. */
export function download(blob: Blob, filename: string) {
	const url = URL.createObjectURL(blob);
	const anchor = document.createElement('a');
	anchor.href = url;
	anchor.download = filename;
	anchor.rel = 'noopener';
	document.body.appendChild(anchor);
	anchor.click();
	// Defer cleanup: revoking the object URL in the same tick as click() can cancel
	// an in-flight download in some browsers.
	setTimeout(() => {
		anchor.remove();
		URL.revokeObjectURL(url);
	}, 1000);
}

/** `download` for in-memory text. */
export const downloadText = (text: string, filename: string, mime: string) =>
	download(new Blob([text], { type: mime }), filename);

/** The short message for a local case whose browser solve failed, and the technical
 * detail to disclose behind it. The engine spells an infeasible solve
 * `DC OPF solve infeasible`, and an unbounded one as `unbounded`. */
export function browserSolveFailure(
	label: string,
	reason: string | null
): { message: string; detail: string | null } {
	const technical = `${label}: browser solve unavailable${reason ? `: ${reason}` : ''}`;
	const infeasible = !!reason && /\binfeasible\b/i.test(reason);
	return { message: `${label}: ${infeasible ? 'infeasible' : 'solve failed'}`, detail: technical };
}

export function rgbaCss([r, g, b, a]: RGBA): string {
	return `rgba(${r}, ${g}, ${b}, ${(a / 255).toFixed(3)})`;
}

/** Min and max of a numeric array in one pass. Used instead of
 * `Math.min(...values)` / `Math.max(...values)`, which throw a RangeError once the
 * argument count exceeds the engine limit (~65k on JavaScriptCore) — reachable on a
 * dropped case with tens of thousands of buses. */
export function extent(values: number[]): { min: number; max: number } {
	let min = Infinity;
	let max = -Infinity;
	for (const v of values) {
		if (v < min) min = v;
		if (v > max) max = v;
	}
	return { min, max };
}

export const fmt = new Intl.NumberFormat('en-US', { maximumFractionDigits: 1 });
/** A number for reading: a true minus sign, and never a negative zero. */
export const plain = (v: number, format: Intl.NumberFormat = fmt) => {
	const text = format.format(Math.abs(v));
	return v < 0 && /[1-9]/.test(text) ? `−${text}` : text;
};
/** A change with its sign. A change that rounds to zero reads "0", with no sign. */
export const signed = (v: number, format: Intl.NumberFormat = fmt) => {
	const text = format.format(Math.abs(v));
	return /[1-9]/.test(text) ? `${v < 0 ? '−' : '+'}${text}` : '0';
};
const twoFigures = new Intl.NumberFormat('en-US', {
	maximumSignificantDigits: 2
});
/** A small signed rate, such as a price change per MW, to two significant
 * figures in plain decimals (0.0017, not 1.7e-3). */
export const signedSmall = (v: number) => signed(Math.abs(v) < SMALL_FLOOR ? 0 : v, twoFigures);
/** The same, without a sign. */
export const small = (v: number) => plain(Math.abs(v) < SMALL_FLOOR ? 0 : v, twoFigures);
/** Below this a rate is solver noise, and reads 0: a price change of a hundredth of a cent
 * per MWh for each MW. */
export const SMALL_FLOOR = 1e-4;
export const signedExp = (v: number) => `${v < 0 ? '−' : '+'}${Math.abs(v).toExponential(2)}`;
const twoDecimals = new Intl.NumberFormat('en-US', {
	minimumFractionDigits: 2,
	maximumFractionDigits: 2
});
/** A price in $/MWh, the same wherever it shows: two decimals, so 22 reads 22.00 beside 20.70. */
export const priceText = (value: number) => plain(value, twoDecimals);
export const displayFmt = (mode: DisplayMode, value: number) =>
	mode === 'price'
		? priceText(value)
		: mode === 'demand'
			? plain(value)
			: plain(value, threeDecimals);
const threeDecimals = new Intl.NumberFormat('en-US', {
	minimumFractionDigits: 3,
	maximumFractionDigits: 3
});

const exactNumber = new Intl.NumberFormat('en', {
	maximumFractionDigits: 6,
	useGrouping: false
});
const decimals = [0, 1, 2, 3, 4].map(
	(d) =>
		new Intl.NumberFormat('en', {
			maximumFractionDigits: d,
			minimumFractionDigits: 0
		})
);

/**
 * A quantity rounded for reading: per-unit voltage to four decimals, percentages to one, and
 * other engineering quantities (MW, MVA, MVAr, prices) to about three significant figures,
 * never dropping integer digits. `exact` keeps up to six decimals, for a tooltip or a check.
 */
export function readable(
	value: number | null | undefined,
	unit: string,
	signed = false
): { text: string; exact: string } {
	if (value == null || !Number.isFinite(value)) return { text: '—', exact: '—' };
	const magnitude = Math.abs(value);
	const places =
		unit === 'pu'
			? 4
			: unit === '%'
				? 1
				: unit === '$/MWh'
					? 2
					: magnitude >= 100
						? 0
						: magnitude >= 10
							? 1
							: 2;
	const rounded = decimals[places].format(magnitude);
	const zero = Number(rounded.replace(/,/g, '')) === 0;
	const sign = !signed || zero ? (value < 0 && !zero ? '-' : '') : value < 0 ? '-' : '+';
	const join = (n: string) => `${n}${unit === '%' ? '' : ' '}${unit}`.trimEnd();
	const exact = exactNumber.format(value);
	return {
		text: join(`${sign}${rounded}`),
		exact: join(signed && value > 0 ? `+${exact}` : exact === '-0' ? '0' : exact)
	};
}

/** The one glyph for a value that is missing or not finite. */
export const MISSING = '—';

/** Units {@link quantity} formats. `V` switches to kV at 1 kV; `°` and `%` attach to the number. */
export type QuantityUnit = 'V' | 'kV' | 'A' | 'kW' | 'kvar' | 'kVA' | '°' | '%' | 'pu' | 'ms' | '';

export interface QuantityOptions {
	/** Prefix positive values with `+`. */
	signed?: boolean;
	/** Up to six decimals in the base unit, no grouping and no kV switch: for a tooltip or a check. */
	exact?: boolean;
	/** Significant figures for the scaled units (V, kV, A, kW, kvar, kVA, ms); default 4. */
	figures?: number;
	/** Fixed decimals, overriding the unit's rule. */
	decimals?: number;
}

const fixedFormats = new Map<number, Intl.NumberFormat>();
function fixedFormat(places: number): Intl.NumberFormat {
	let f = fixedFormats.get(places);
	if (!f) {
		f = new Intl.NumberFormat('en-US', {
			minimumFractionDigits: places,
			maximumFractionDigits: places
		});
		fixedFormats.set(places, f);
	}
	return f;
}

/** Decimals for a unit: pu to four, angles to two, percent to one, everything else to `figures`
 * significant figures without ever dropping integer digits. */
function placesFor(magnitude: number, unit: QuantityUnit, figures: number): number {
	if (unit === 'pu') return 4;
	if (unit === '°') return 2;
	if (unit === '%') return 1;
	const integerDigits = magnitude >= 1 ? Math.floor(Math.log10(magnitude)) + 1 : 1;
	return Math.min(figures - 1, Math.max(0, figures - integerDigits));
}

/**
 * The one formatter for a multiconductor or balanced quantity on screen: thousands separators,
 * `figures` significant figures (four by default) for V, A, kW, kvar, kVA and ms, two decimals for
 * degrees, one for percent and four for pu. Volts switch to kV at |v| ≥ 1000. `%` and `°` attach to
 * the number, other units follow a space. A missing or non-finite value is {@link MISSING}.
 */
export function quantity(
	value: number | null | undefined,
	unit: QuantityUnit,
	options: QuantityOptions = {}
): string {
	if (value == null || !Number.isFinite(value)) return MISSING;
	const { signed = false, exact = false, figures = 4, decimals } = options;
	let shown = value;
	let shownUnit: string = unit;
	if (!exact && unit === 'V' && Math.abs(value) >= 1000) {
		shown = value / 1000;
		shownUnit = 'kV';
	}
	const attach = shownUnit === '%' || shownUnit === '°' || shownUnit === '';
	const join = (n: string) => (attach ? `${n}${shownUnit}` : `${n} ${shownUnit}`);
	if (exact) {
		const text = exactNumber.format(shown);
		return join(text === '-0' ? '0' : signed && shown > 0 ? `+${text}` : text);
	}
	const magnitude = Math.abs(shown);
	const places = decimals ?? placesFor(magnitude, shownUnit as QuantityUnit, figures);
	const rounded = fixedFormat(places).format(magnitude);
	const zero = Number(rounded.replace(/,/g, '')) === 0;
	const sign = zero ? '' : shown < 0 ? '-' : signed ? '+' : '';
	return join(`${sign}${rounded}`);
}

/** A voltage with its per-unit value beside it when the base is known: `239.6 V (0.9983 pu)`. */
export function voltsWithPu(volts: number | null | undefined, base: number | null | undefined) {
	const v = quantity(volts, 'V');
	if (volts == null || !Number.isFinite(volts) || !base || !Number.isFinite(base) || base <= 0)
		return v;
	return `${v} (${quantity(volts / base, 'pu')})`;
}

/** Degrees from radians, through {@link quantity}. */
export const degrees = (radians: number | null | undefined, options?: QuantityOptions) =>
	quantity(radians == null ? radians : (radians * 180) / Math.PI, '°', options);

/** A range through {@link quantity}, the unit written once when both ends share it:
 * `203.4–239.6 V`, `0.9500–1.0500 pu`, `950.0 V – 1.050 kV`. */
export function quantityRange(
	low: number | null | undefined,
	high: number | null | undefined,
	unit: QuantityUnit
): string {
	const a = quantity(low, unit);
	const b = quantity(high, unit);
	if (a === MISSING || b === MISSING) return a === b ? MISSING : `${a} – ${b}`;
	const suffix = (s: string) => s.match(/\s?[^\d\s.,+-]*$/)?.[0] ?? '';
	const sa = suffix(a);
	return sa && sa === suffix(b) ? `${a.slice(0, -sa.length)}–${b}` : `${a} – ${b}`;
}
