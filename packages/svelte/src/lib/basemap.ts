import type { StyleSpecification } from 'maplibre-gl';

/** Basemap selection, fallback and tuning for `TellegenMap`.
 *
 * CARTO's vector styles sit quietly under a network overlay, and CARTO's
 * basemap terms ask for an API key (free for non-commercial use, from
 * carto.com/basemaps/apikey). With a key, the CARTO style leads and
 * OpenFreeMap (no key, CORS open) is the fallback. Without one, OpenFreeMap
 * leads and the keyless CARTO style is the fallback. Whenever an OpenFreeMap
 * style renders, `openFreeMapEdits` tunes it towards CARTO's look. A host can
 * replace either style with its own URL or inline `StyleSpecification`, or
 * disable the fallback with `null`. */

export type BasemapTheme = 'light' | 'dark';
export type BasemapStyle = string | StyleSpecification;

/** `TellegenMap`'s `theme` prop: the basemap theme, which also picks the
 * overlay palette. */
export type MapTheme = BasemapTheme;
/** `TellegenMap`'s `projection` prop. MapLibre's globe falls back to
 * Mercator at high zoom on its own. */
export type MapProjection = 'mercator' | 'globe';

/** Who serves a basemap style: one of the two default providers, or
 * `'custom'` for a style the host supplied. */
export type BasemapProvider = 'carto' | 'openfreemap' | 'custom';

export interface BasemapSpec {
	primary: BasemapStyle;
	fallback: BasemapStyle | null;
	/** Hide the style's `symbol` layers after every `style.load`. */
	hideLabels: boolean;
	/** Who serves `primary`. Absent counts as `'custom'`. */
	primaryProvider?: BasemapProvider;
	/** Who serves `fallback`, `null` with no fallback. Absent counts as
	 * `'custom'`. */
	fallbackProvider?: BasemapProvider | null;
}

/** A `BasemapSpec` that names both providers, as `defaultBasemap` and
 * `resolveBasemap` return it. A host words its credits and fallback notice
 * from these instead of reading style URLs. */
export interface ResolvedBasemap extends BasemapSpec {
	primaryProvider: BasemapProvider;
	fallbackProvider: BasemapProvider | null;
}

/** What `defaultBasemap` returns: one CARTO and one OpenFreeMap style URL,
 * in the order the key decides. Never `'custom'`, and never without a
 * fallback. */
export interface DefaultBasemap extends ResolvedBasemap {
	primary: string;
	fallback: string;
	primaryProvider: 'carto' | 'openfreemap';
	fallbackProvider: 'carto' | 'openfreemap';
}

export const OPENFREEMAP_STYLES = {
	light: 'https://tiles.openfreemap.org/styles/positron',
	dark: 'https://tiles.openfreemap.org/styles/dark'
} as const;

export const CARTO_STYLES = {
	light: 'https://basemaps.cartocdn.com/gl/positron-gl-style/style.json',
	'light-nolabels': 'https://basemaps.cartocdn.com/gl/positron-nolabels-gl-style/style.json',
	dark: 'https://basemaps.cartocdn.com/gl/dark-matter-gl-style/style.json',
	'dark-nolabels': 'https://basemaps.cartocdn.com/gl/dark-matter-nolabels-gl-style/style.json'
} as const;

/** A CARTO key with surrounding whitespace removed, or `undefined` when
 * nothing is left. A variable saved with a stray newline or space must not
 * count as a key. */
export function normalizeCartoApiKey(apiKey: string | undefined): string | undefined {
	return apiKey?.trim() || undefined;
}

/** A CARTO style URL with the optional API key appended as `key=`. A blank
 * key appends nothing. */
export function cartoStyleUrl(style: string, apiKey?: string): string {
	const key = normalizeCartoApiKey(apiKey);
	if (!key) return style;
	const separator = style.includes('?') ? '&' : '?';
	return `${style}${separator}key=${encodeURIComponent(key)}`;
}

export interface DefaultBasemapOptions {
	/** CARTO basemap key, sent as `key=`. With one, CARTO leads and
	 * OpenFreeMap is the fallback; without one, the order flips. The key is
	 * trimmed, and a blank one counts as none. Restrict the key to the hosts
	 * that serve your app. */
	cartoApiKey?: string;
	/** Default `true`: the CARTO style uses its `-nolabels` variant, and label
	 * layers are hidden on load whichever style renders. */
	hideLabels?: boolean;
}

/** The default primary and fallback for a theme, ordered by whether a CARTO
 * key is at hand. */
export function defaultBasemap(
	theme: BasemapTheme,
	options: DefaultBasemapOptions = {}
): DefaultBasemap {
	const hideLabels = options.hideLabels ?? true;
	const variant = hideLabels ? (`${theme}-nolabels` as const) : theme;
	const key = normalizeCartoApiKey(options.cartoApiKey);
	const carto = cartoStyleUrl(CARTO_STYLES[variant], key);
	const openFreeMap = OPENFREEMAP_STYLES[theme];
	return key
		? {
				primary: carto,
				fallback: openFreeMap,
				hideLabels,
				primaryProvider: 'carto',
				fallbackProvider: 'openfreemap'
			}
		: {
				primary: openFreeMap,
				fallback: carto,
				hideLabels,
				primaryProvider: 'openfreemap',
				fallbackProvider: 'carto'
			};
}

export interface ResolveBasemapInput {
	/** Overrides the default primary. */
	mapStyle?: BasemapStyle;
	/** Overrides the default fallback; `null` disables the fallback. */
	fallbackMapStyle?: BasemapStyle | null;
	theme: BasemapTheme;
	hideBasemapLabels: boolean;
	cartoApiKey?: string;
}

/** The spec `TellegenMap` renders from its props: explicit styles win, the
 * theme fills in whatever is left, and `null` turns the fallback off. An
 * explicit style is `'custom'`, whichever host serves it. */
export function resolveBasemap(input: ResolveBasemapInput): ResolvedBasemap {
	const defaults = defaultBasemap(input.theme, {
		cartoApiKey: input.cartoApiKey,
		hideLabels: input.hideBasemapLabels
	});
	const fallback = input.fallbackMapStyle;
	return {
		primary: input.mapStyle ?? defaults.primary,
		fallback: fallback === undefined ? defaults.fallback : fallback,
		hideLabels: input.hideBasemapLabels,
		primaryProvider: input.mapStyle === undefined ? defaults.primaryProvider : 'custom',
		fallbackProvider:
			fallback === undefined ? defaults.fallbackProvider : fallback === null ? null : 'custom'
	};
}

const OPENFREEMAP_TILES = 'https://tiles.openfreemap.org/';
/** Road lines start here on an OpenFreeMap style. Below it, the road web
 * reads like grid lines under a network overlay. */
const ROAD_MIN_ZOOM = 8;
/** CARTO Dark Matter's water and waterway colours, which stand clear of
 * near-black land. */
const DARK_WATER = '#2c353c';
const DARK_WATERWAY = '#3f5a6d';
const LAND_SOURCE_LAYERS = new Set(['landuse', 'landcover', 'park']);

/** Whether a style draws on OpenFreeMap's tiles, whichever URL loaded it. */
export function isOpenFreeMapStyle(style: StyleSpecification): boolean {
	return Object.values(style.sources ?? {}).some((source) => {
		const urls = [
			'url' in source ? source.url : undefined,
			...('tiles' in source ? (source.tiles ?? []) : [])
		];
		return urls.some((url) => typeof url === 'string' && url.startsWith(OPENFREEMAP_TILES));
	});
}

/** One change to a loaded style, applied through the map's own API. */
export type StyleEdit =
	| { kind: 'zoom-range'; layer: string; minzoom: number; maxzoom: number }
	| { kind: 'paint'; layer: string; property: 'fill-color' | 'line-color'; value: string };

/** Edits that let an OpenFreeMap style sit under a network overlay the way
 * CARTO's styles do.
 *
 * - Road lines (`transportation`) start at z8.
 * - In the dark theme, water and waterways take Dark Matter's colours, and
 *   plain land fills (`landuse`, `landcover`, `park`) take the style's
 *   background colour, so the water stands out against one tone of land.
 *
 * Layers are matched by `source-layer`, not by id, so the edits still apply
 * when OpenFreeMap renames its layers. */
export function openFreeMapEdits(style: StyleSpecification, theme: BasemapTheme): StyleEdit[] {
	const paintOf = (layer: StyleSpecification['layers'][number]) =>
		(layer.paint ?? {}) as Record<string, unknown>;
	const background = style.layers.find((layer) => layer.type === 'background');
	const backgroundColor = background ? paintOf(background)['background-color'] : undefined;
	// A plain colour only; an expression is left alone rather than guessed at.
	const land = typeof backgroundColor === 'string' ? backgroundColor : undefined;
	const edits: StyleEdit[] = [];
	for (const layer of style.layers) {
		const sourceLayer = 'source-layer' in layer ? layer['source-layer'] : undefined;
		if (sourceLayer === undefined) continue;
		if (sourceLayer === 'transportation' && layer.type === 'line') {
			if ((layer.minzoom ?? 0) < ROAD_MIN_ZOOM) {
				edits.push({
					kind: 'zoom-range',
					layer: layer.id,
					minzoom: ROAD_MIN_ZOOM,
					maxzoom: layer.maxzoom ?? 24
				});
			}
		} else if (theme === 'dark' && sourceLayer === 'water' && layer.type === 'fill') {
			edits.push({ kind: 'paint', layer: layer.id, property: 'fill-color', value: DARK_WATER });
		} else if (theme === 'dark' && sourceLayer === 'waterway' && layer.type === 'line') {
			edits.push({ kind: 'paint', layer: layer.id, property: 'line-color', value: DARK_WATERWAY });
		} else if (
			theme === 'dark' &&
			land !== undefined &&
			layer.type === 'fill' &&
			LAND_SOURCE_LAYERS.has(sourceLayer) &&
			paintOf(layer)['fill-pattern'] === undefined
		) {
			edits.push({ kind: 'paint', layer: layer.id, property: 'fill-color', value: land });
		}
	}
	return edits;
}

/** The providers a fallback event concerns. */
export interface BasemapSwitch {
	/** The provider whose style failed. */
	from: BasemapProvider;
	/** The provider switched to; `null` when no basemap is left to show. */
	to: BasemapProvider | null;
}

type BasemapFailure =
	| { kind: 'style-load'; message: string; status?: number }
	| { kind: 'source-forbidden'; message: string; status: number; sourceId: string }
	| { kind: 'tiles-unavailable'; message: string; sourceId: string }
	| { kind: 'webgl'; message: string };

export type BasemapFallbackReason = BasemapFailure & BasemapSwitch;

/** The fields of a MapLibre `error` or `sourcedata` event the fallback reads. */
export interface BasemapMapEvent {
	error?: unknown;
	sourceId?: string;
	tile?: unknown;
	dataType?: string;
	isSourceLoaded?: boolean;
}

/** The slice of `maplibregl.Map` the fallback needs; a test drives a fake. */
export interface BasemapMapLike {
	on(type: string, listener: (event: BasemapMapEvent) => void): unknown;
	off(type: string, listener: (event: BasemapMapEvent) => void): unknown;
	setStyle(style: BasemapStyle, options?: { diff?: boolean }): unknown;
	/** MapLibre types this `boolean | void`; only `true` counts as loaded. */
	isStyleLoaded(): boolean | void;
}

export interface BasemapFallbackHandlers {
	/** The primary failed and the fallback style was requested. `from` is the
	 * spec's primary provider and `to` its fallback provider. */
	onFallback?(reason: BasemapFallbackReason): void;
	/** No basemap can be shown: the fallback failed too, none was configured,
	 * or WebGL is unavailable. `from` is the provider on screen, `to` is
	 * `null`. */
	onFailure?(reason: BasemapFallbackReason): void;
}

export interface BasemapFallbackOptions {
	/** Clock for the tile failure window; tests inject one. */
	now?: () => number;
}

/** Tile fetch failures from one source, with no tile of any source arriving,
 * that trip the fallback inside the window. */
const TILE_FAILURE_LIMIT = 6;
const TILE_FAILURE_WINDOW_MS = 10_000;
const FORBIDDEN_STATUSES = new Set([401, 403, 429]);
const FORBIDDEN_MESSAGE = /api key|forbidden|unauthorized/i;
const NETWORK_MESSAGE = /failed to fetch|load failed|network ?error|network request failed/i;
const WEBGL_MESSAGE = /webgl/i;

function errorMessage(error: unknown): string {
	if (error instanceof Error) return error.message;
	if (typeof error === 'string') return error;
	if (error && typeof error === 'object' && 'message' in error) return String(error.message);
	return error === undefined || error === null ? '' : String(error);
}

function errorStatus(error: unknown): number | undefined {
	if (error && typeof error === 'object' && 'status' in error) {
		const status = (error as { status?: unknown }).status;
		if (typeof status === 'number') return status;
	}
	return undefined;
}

/** Watch a map's `error` stream and switch to `spec.fallback` when the
 * primary basemap cannot be shown. Rules, in order:
 *
 * 1. an error with no `sourceId` before the first `style.load` is a style
 *    fetch failure: switch to the fallback once;
 * 2. a source or tile error with status 401, 403 or 429, or a message naming
 *    an API key, forbidden or unauthorized access: switch;
 * 3. more than six tile fetch failures within ten seconds while no tile of
 *    any source has arrived: switch. A failure is a network error with no
 *    status, or an HTTP 5xx answer;
 * 4. a WebGL error: report failure, no style can help;
 * 5. everything else is logged the way MapLibre logs an unhandled error,
 *    because registering this listener silences that default.
 *
 * After the switch the same rules report `onFailure` instead of switching
 * again, as they do when `spec.fallback` is `null`. Returns a detach
 * function. */
export function attachBasemapFallback(
	map: BasemapMapLike,
	spec: BasemapSpec,
	handlers: BasemapFallbackHandlers = {},
	options: BasemapFallbackOptions = {}
): () => void {
	const now = options.now ?? (() => Date.now());
	const primaryProvider = spec.primaryProvider ?? 'custom';
	const fallbackProvider = spec.fallback === null ? null : (spec.fallbackProvider ?? 'custom');
	let styleLoaded = map.isStyleLoaded() === true;
	let fellBack = false;
	let failed = false;
	let tileFailures = 0;
	let tileWindowStart = 0;
	let tileArrived = false;

	const fail = (reason: BasemapFailure) => {
		if (failed) return;
		failed = true;
		const from = fellBack && fallbackProvider !== null ? fallbackProvider : primaryProvider;
		handlers.onFailure?.({ ...reason, from, to: null });
	};
	const fallBack = (reason: BasemapFailure) => {
		if (fellBack || spec.fallback === null) {
			fail(reason);
			return;
		}
		fellBack = true;
		styleLoaded = false;
		tileFailures = 0;
		tileArrived = false;
		handlers.onFallback?.({ ...reason, from: primaryProvider, to: fallbackProvider });
		map.setStyle(spec.fallback, { diff: false });
	};
	const passThrough = (event: BasemapMapEvent) => {
		console.error(event?.error ?? event ?? 'Empty error event');
	};

	const onStyleLoad = () => {
		styleLoaded = true;
	};
	const onSourceData = (event: BasemapMapEvent) => {
		if (event?.tile || event?.isSourceLoaded === true) tileArrived = true;
	};
	const onError = (event: BasemapMapEvent) => {
		const message = errorMessage(event?.error);
		const status = errorStatus(event?.error);
		if (WEBGL_MESSAGE.test(message)) {
			fail({ kind: 'webgl', message });
			return;
		}
		const sourceId = typeof event?.sourceId === 'string' ? event.sourceId : undefined;
		if (sourceId === undefined) {
			if (!styleLoaded) {
				fallBack({ kind: 'style-load', message, status });
				return;
			}
			passThrough(event);
			return;
		}
		if (
			(status !== undefined && FORBIDDEN_STATUSES.has(status)) ||
			FORBIDDEN_MESSAGE.test(message)
		) {
			fallBack({ kind: 'source-forbidden', message, status: status ?? 0, sourceId });
			return;
		}
		const transient =
			status === undefined || status === 0 ? NETWORK_MESSAGE.test(message) : status >= 500;
		if (event.tile && transient) {
			const t = now();
			if (tileFailures === 0 || t - tileWindowStart > TILE_FAILURE_WINDOW_MS) {
				tileWindowStart = t;
				tileFailures = 0;
				tileArrived = false;
			}
			tileFailures++;
			if (tileFailures > TILE_FAILURE_LIMIT && !tileArrived) {
				fallBack({ kind: 'tiles-unavailable', message, sourceId });
				return;
			}
		}
		passThrough(event);
	};

	map.on('style.load', onStyleLoad);
	map.on('sourcedata', onSourceData);
	map.on('error', onError);
	return () => {
		map.off('style.load', onStyleLoad);
		map.off('sourcedata', onSourceData);
		map.off('error', onError);
	};
}
