/** File name helpers for drop routing. JSON content classification lives in
 * powerio's Rust/Wasm `classify_json_bytes` path, not in this TypeScript layer. */

/** The distribution format for a file recognized by extension, or null. Only
 * `.dss` is name routed; PMD and BMOPF share the `.json` extension and are
 * classified by the engine. */
export function distExtensionFormat(name: string): 'dss' | null {
	return name.split('.').pop()?.toLowerCase() === 'dss' ? 'dss' : null;
}

/** True for geographic sidecar files by extension (`.csv`, `.json`,
 * `.geojson`). Routable `.json` content (packages, distribution documents) is
 * consumed by the engine classifier before this applies; what remains parses
 * through the engine's tolerant geo reader. */
export function isGeoFileName(name: string): boolean {
	const dot = name.lastIndexOf('.');
	if (dot <= 0) return false;
	const ext = name.slice(dot + 1).toLowerCase();
	return ext === 'csv' || ext === 'json' || ext === 'geojson';
}
