/** The single owner of drop-file classification: given a file name or its JSON
 * content, decide which ingest path handles it. Extension routing (`.dss`, and
 * the balanced `.m`/`.raw`/`.aux` via `formatOf` in the engine) is name-based;
 * everything under `.json` is content-sniffed here so there is one place that
 * decides package-vs-case and balanced-vs-multiconductor.
 *
 * The rules mirror the readers they feed:
 *   - a `.pio.json` package is recognized by either envelope spelling (see
 *     {@link packageKind}), then split by kind (balanced restores a study;
 *     multiconductor is viewed);
 *   - a non-package document is distribution JSON, PMD when it carries the
 *     `data_model` marker and BMOPF otherwise (the same split
 *     `powerio_dist` uses for `.json`), except a GeoJSON FeatureCollection,
 *     which stays unrouted so the geo sidecar path keeps accepting it.
 *
 * Every function is total: malformed, truncated, or non-JSON input classifies
 * as `not-json` rather than throwing. */

/** The schema URL that a `.pio.json` envelope carried through powerio 0.7.x.
 * Later versions removed the field and kept `schema_version` alone. */
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

/** True for geographic sidecar files by extension (`.csv`, `.json`,
 * `.geojson`). Routable `.json` content (packages, distribution documents) is
 * consumed by {@link classifyJson} before this applies; what remains parses
 * through the engine's tolerant geo reader. */
export function isGeoFileName(name: string): boolean {
	const dot = name.lastIndexOf('.');
	if (dot <= 0) return false;
	const ext = name.slice(dot + 1).toLowerCase();
	return ext === 'csv' || ext === 'json' || ext === 'geojson';
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

/** The model family of a `.pio.json` envelope, or null when `obj` is not one.
 *
 * powerio replaced the envelope's four version identifiers with one
 * `schema_version`, so two spellings exist and both must classify:
 *   - through 0.7.x, a `schema` URL under {@link PIO_PACKAGE_SCHEMA_PREFIX};
 *   - after that, `schema_version` alone.
 *
 * The second rule needs a model kind beside the version. A case document can
 * carry a `schema_version`, but only an envelope carries both. */
export function packageKind(
	obj: Record<string, unknown> | null
): 'balanced' | 'multiconductor' | null {
	if (!obj) return null;
	const kind = packageModelKind(obj);
	if (typeof obj.schema === 'string' && obj.schema.startsWith(PIO_PACKAGE_SCHEMA_PREFIX)) {
		// A 0.7.x package with no readable kind is balanced. That is the
		// historical payload, and the study-restore path fails closed anyway.
		return kind ?? 'balanced';
	}
	return typeof obj.schema_version === 'string' ? kind : null;
}

/** Classify a dropped JSON document into its ingest path. */
export function classifyJson(text: string): JsonDropKind {
	const obj = topLevelObject(text);
	if (!obj) return 'not-json';
	const kind = packageKind(obj);
	if (kind) return `${kind}-package`;
	// A GeoJSON FeatureCollection saved as `.json` is coordinate data for the
	// geo sidecar path, which accepted it before this classifier existed; leave
	// it unrouted so it falls through.
	if (Array.isArray((obj as { features?: unknown }).features)) return 'not-json';
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
