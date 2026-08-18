/** The single owner of drop-file classification: given a file name or its JSON
 * content, decide which ingest path handles it. Extension routing (`.dss`, and
 * the balanced `.m`/`.raw`/`.aux` via `formatOf` in the engine) is name-based;
 * everything under `.json` is content-sniffed here so there is one place that
 * decides package-vs-case and balanced-vs-multiconductor.
 *
 * The rules mirror the readers they feed, and the package and model JSON rules
 * are powerio's own `classify_json_text` written out in TypeScript:
 *   - a `.pio.json` package is recognized by a `model_kind` of `balanced` or
 *     `multiconductor` beside a `model` key, which then splits it (balanced
 *     restores a study; multiconductor is viewed). The value check keeps a case
 *     document that happens to carry those key names from being misrouted;
 *   - bare model JSON, the object powerio's `to_json` writes, is recognized by
 *     `buses` beside another network key, which the case formats spell
 *     differently (PowerModels writes `bus`, not `buses`);
 *   - a non-package document is distribution JSON, PMD when it carries the
 *     `data_model` marker and BMOPF otherwise (the same split
 *     `powerio_dist` uses for `.json`), except a GeoJSON FeatureCollection,
 *     which stays unrouted so the geo sidecar path keeps accepting it.
 *
 * Every function is total: malformed, truncated, or non-JSON input classifies
 * as `not-json` rather than throwing. */

/** How a dropped JSON file should be ingested. */
export type JsonDropKind =
	/** A saved balanced study package: restore the case, edits, and formulation. */
	| 'balanced-package'
	/** A package carrying a multiconductor payload: view it (no solve). */
	| 'multiconductor-package'
	/** Bare powerio model JSON: a balanced network with no package envelope. */
	| 'model-json'
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

/** Whether `obj` is a `.pio.json` package envelope. The marker is powerio's:
 * a `model_kind` naming a model family beside the `model` key that carries it.
 * powerio stopped writing the `schema` URL field in 0.8.0, so a package is
 * identified by what it declares about its payload, not by a schema id. */
export function isPackageEnvelope(obj: Record<string, unknown> | null): boolean {
	const kind = obj?.model_kind;
	return (kind === 'balanced' || kind === 'multiconductor') && 'model' in (obj ?? {});
}

/** Whether `obj` is bare powerio model JSON: `buses` beside another network
 * key. The case formats spell that table differently (PowerModels writes
 * `bus`), so the plural is what separates powerio's own document from a case. */
function isModelJson(obj: Record<string, unknown>): boolean {
	if (!Array.isArray(obj.buses)) return false;
	return ['branches', 'generators', 'loads', 'shunts', 'base_mva'].some((key) => key in obj);
}

/** Classify a dropped JSON document into its ingest path. */
export function classifyJson(text: string): JsonDropKind {
	const obj = topLevelObject(text);
	if (!obj) return 'not-json';
	// `isPackageEnvelope` already checked that `model_kind` names one of the two
	// families, so it splits the package on its own.
	if (isPackageEnvelope(obj)) {
		return obj.model_kind === 'multiconductor' ? 'multiconductor-package' : 'balanced-package';
	}
	// A GeoJSON FeatureCollection saved as `.json` is coordinate data for the
	// geo sidecar path, which accepted it before this classifier existed; leave
	// it unrouted so it falls through.
	if (Array.isArray((obj as { features?: unknown }).features)) return 'not-json';
	// powerio's own model document, which is not a case format: it goes to the
	// balanced path rather than to a distribution reader.
	if (isModelJson(obj)) return 'model-json';
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
