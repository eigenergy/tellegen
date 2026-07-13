/** The single owner of drop-file classification: given a file name or its JSON
 * content, decide which ingest path handles it. Extension routing (`.dss`, and
 * the balanced `.m`/`.raw`/`.aux` via `formatOf` in the engine) is name-based;
 * everything under `.json` is content-sniffed here so there is one place that
 * decides package-vs-case and balanced-vs-multiconductor.
 *
 * The rules mirror the readers they feed:
 *   - a `.pio.json` package is recognized by its `schema` envelope field, then
 *     split by the authoritative `model_kind` (balanced restores a study;
 *     multiconductor is viewed);
 *   - a non-package document is distribution JSON, PMD when it carries the
 *     `data_model` marker and BMOPF otherwise (the same split
 *     `powerio_dist` uses for `.json`).
 *
 * Every function is total: malformed, truncated, or non-JSON input classifies
 * as `not-json` rather than throwing. */

/** The schema URL prefix every `.pio.json` package envelope carries. */
export const PIO_PACKAGE_SCHEMA_PREFIX = 'https://powerio.dev/schema/pio-package';

/** How a dropped JSON file should be ingested. */
export type JsonDropKind =
	/** A saved balanced study package: restore the case, edits, and formulation. */
	| 'balanced-package'
	/** A package carrying a multiconductor payload: view it (no solve). */
	| 'multiconductor-package'
	/** A BMOPF JSON distribution case. */
	| 'bmopf'
	/** A PowerModelsDistribution ENGINEERING JSON case. */
	| 'pmd'
	/** Not JSON, or JSON we do not route (left to the generic error path). */
	| 'not-json';

/** The distribution format for a file recognized by extension, or null. Only
 * `.dss` is name-routed; PMD and BMOPF share the `.json` extension and are
 * content-sniffed by {@link classifyJson}. */
export function distExtensionFormat(name: string): 'dss' | null {
	return name.split('.').pop()?.toLowerCase() === 'dss' ? 'dss' : null;
}

/** Parse `text` as JSON, returning the top-level object or null. */
function topLevelObject(text: string): Record<string, unknown> | null {
	let value: unknown;
	try {
		value = JSON.parse(text);
	} catch {
		return null;
	}
	return typeof value === 'object' && value !== null && !Array.isArray(value)
		? (value as Record<string, unknown>)
		: null;
}

/** Whether `text` is a `.pio.json` package envelope, by its `schema` field. */
export function isPackageEnvelope(obj: Record<string, unknown> | null): boolean {
	return typeof obj?.schema === 'string' && obj.schema.startsWith(PIO_PACKAGE_SCHEMA_PREFIX);
}

/** The package's model family. `model_kind` is authoritative and stored
 * explicitly; fall back to the payload's own `model.kind` tag for a package
 * written before the field existed. */
function packageModelKind(obj: Record<string, unknown>): 'balanced' | 'multiconductor' | null {
	const explicit = obj.model_kind;
	if (explicit === 'balanced' || explicit === 'multiconductor') return explicit;
	const payloadKind = (obj.model as { kind?: unknown } | undefined)?.kind;
	if (payloadKind === 'balanced' || payloadKind === 'multiconductor') return payloadKind;
	return null;
}

/** Classify a dropped JSON document into its ingest path. */
export function classifyJson(text: string): JsonDropKind {
	const obj = topLevelObject(text);
	if (!obj) return 'not-json';
	if (isPackageEnvelope(obj)) {
		const kind = packageModelKind(obj);
		// A package with no readable kind is treated as balanced: that is the
		// historical payload and the study-restore path fails closed on a
		// non-balanced document anyway.
		return kind === 'multiconductor' ? 'multiconductor-package' : 'balanced-package';
	}
	// Non-package JSON is a distribution document: PMD declares `data_model`,
	// everything else is routed to BMOPF (which gives the precise parse error
	// when it is neither).
	return 'data_model' in obj ? 'pmd' : 'bmopf';
}

/** Whether `text` is a saved study package (a balanced `.pio.json`). Kept for
 * the study-restore path; multiconductor packages are viewed, not restored. */
export function isStudyPackageText(text: string): boolean {
	return classifyJson(text) === 'balanced-package';
}
