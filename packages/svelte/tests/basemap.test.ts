import { readFileSync } from 'node:fs';
import type { StyleSpecification } from 'maplibre-gl';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	CARTO_STYLES,
	OPENFREEMAP_STYLES,
	attachBasemapFallback,
	cartoStyleUrl,
	defaultBasemap,
	isOpenFreeMapStyle,
	openFreeMapEdits,
	resolveBasemap,
	type BasemapFallbackReason,
	type BasemapMapEvent,
	type BasemapMapLike,
	type BasemapSpec,
	type BasemapStyle
} from '../src/lib/basemap.js';
import { resolveTellegenUiConfig } from '../src/lib/context.svelte.js';

type Listener = (event: BasemapMapEvent) => void;

/** The slice of a MapLibre map the fallback touches, driven by hand. */
class FakeMap implements BasemapMapLike {
	listeners = new Map<string, Set<Listener>>();
	styles: { style: BasemapStyle; options?: { diff?: boolean } }[] = [];
	loaded = false;
	on(type: string, listener: Listener) {
		let set = this.listeners.get(type);
		if (!set) this.listeners.set(type, (set = new Set()));
		set.add(listener);
		return this;
	}
	off(type: string, listener: Listener) {
		this.listeners.get(type)?.delete(listener);
		return this;
	}
	setStyle(style: BasemapStyle, options?: { diff?: boolean }) {
		this.styles.push({ style, options });
		this.loaded = false;
		return this;
	}
	isStyleLoaded() {
		return this.loaded;
	}
	emit(type: string, event: BasemapMapEvent = {}) {
		if (type === 'style.load') this.loaded = true;
		for (const listener of [...(this.listeners.get(type) ?? [])]) listener(event);
	}
	listenerCount() {
		let n = 0;
		for (const set of this.listeners.values()) n += set.size;
		return n;
	}
}

/** MapLibre's AJAXError shape: `status` 0 for a network failure, else HTTP. */
function ajaxError(status: number, message: string) {
	return Object.assign(new Error(message), { status, url: 'https://tiles.example/0/0/0.pbf' });
}

const spec: BasemapSpec = {
	primary: 'https://primary.example/style.json',
	fallback: 'https://fallback.example/style.json',
	hideLabels: true,
	primaryProvider: 'openfreemap',
	fallbackProvider: 'carto'
};

function attach(map: FakeMap, overrides: Partial<BasemapSpec> = {}, now?: () => number) {
	const onFallback = vi.fn<(r: BasemapFallbackReason) => void>();
	const onFailure = vi.fn<(r: BasemapFallbackReason) => void>();
	const detach = attachBasemapFallback(
		map,
		{ ...spec, ...overrides },
		{ onFallback, onFailure },
		now ? { now } : {}
	);
	return { onFallback, onFailure, detach };
}

describe('style URLs', () => {
	it('appends the CARTO key with the right separator', () => {
		expect(cartoStyleUrl(CARTO_STYLES.dark)).toBe(CARTO_STYLES.dark);
		expect(cartoStyleUrl(CARTO_STYLES.dark, 'abc 1')).toBe(`${CARTO_STYLES.dark}?key=abc%201`);
		expect(cartoStyleUrl('https://x.example/s.json?v=2', 'k')).toBe(
			'https://x.example/s.json?v=2&key=k'
		);
	});

	it('without a CARTO key, leads with OpenFreeMap and falls back to keyless CARTO', () => {
		expect(defaultBasemap('light')).toEqual({
			primary: OPENFREEMAP_STYLES.light,
			fallback: CARTO_STYLES['light-nolabels'],
			hideLabels: true,
			primaryProvider: 'openfreemap',
			fallbackProvider: 'carto'
		});
		expect(defaultBasemap('dark', { hideLabels: false, cartoApiKey: '' })).toEqual({
			primary: OPENFREEMAP_STYLES.dark,
			fallback: CARTO_STYLES.dark,
			hideLabels: false,
			primaryProvider: 'openfreemap',
			fallbackProvider: 'carto'
		});
	});

	it('with a CARTO key, leads with the keyed CARTO style and falls back to OpenFreeMap', () => {
		expect(defaultBasemap('light', { cartoApiKey: 'k' })).toEqual({
			primary: `${CARTO_STYLES['light-nolabels']}?key=k`,
			fallback: OPENFREEMAP_STYLES.light,
			hideLabels: true,
			primaryProvider: 'carto',
			fallbackProvider: 'openfreemap'
		});
		expect(defaultBasemap('dark', { cartoApiKey: 'k' })).toEqual({
			primary: `${CARTO_STYLES['dark-nolabels']}?key=k`,
			fallback: OPENFREEMAP_STYLES.dark,
			hideLabels: true,
			primaryProvider: 'carto',
			fallbackProvider: 'openfreemap'
		});
		expect(defaultBasemap('dark', { hideLabels: false, cartoApiKey: 'k' }).primary).toBe(
			`${CARTO_STYLES.dark}?key=k`
		);
		expect(resolveBasemap({ theme: 'light', hideBasemapLabels: true, cartoApiKey: 'k' })).toEqual(
			defaultBasemap('light', { cartoApiKey: 'k' })
		);
	});

	it('always defaults to one CARTO and one OpenFreeMap URL, never custom or without a fallback', () => {
		for (const theme of ['light', 'dark'] as const) {
			for (const cartoApiKey of [undefined, '', 'k']) {
				for (const hideLabels of [true, false]) {
					const spec = defaultBasemap(theme, { cartoApiKey, hideLabels });
					expect(typeof spec.primary).toBe('string');
					expect(typeof spec.fallback).toBe('string');
					expect(new Set([spec.primaryProvider, spec.fallbackProvider])).toEqual(
						new Set(['carto', 'openfreemap'])
					);
					const cartoUrl = spec.primaryProvider === 'carto' ? spec.primary : spec.fallback;
					expect(cartoUrl.startsWith('https://basemaps.cartocdn.com/')).toBe(true);
				}
			}
		}
	});

	it('trims the CARTO key, and a blank one counts as none', () => {
		for (const blank of ['  ', '\n']) {
			expect(defaultBasemap('light', { cartoApiKey: blank })).toEqual(defaultBasemap('light'));
			expect(resolveBasemap({ theme: 'dark', hideBasemapLabels: true, cartoApiKey: blank })).toEqual(
				defaultBasemap('dark')
			);
			expect(cartoStyleUrl(CARTO_STYLES.dark, blank)).toBe(CARTO_STYLES.dark);
			expect(resolveTellegenUiConfig({ cartoApiKey: blank }).cartoApiKey).toBe('');
		}
		const keyed = defaultBasemap('light', { cartoApiKey: ' abc ' });
		expect(keyed).toMatchObject({
			primary: `${CARTO_STYLES['light-nolabels']}?key=abc`,
			primaryProvider: 'carto',
			fallbackProvider: 'openfreemap'
		});
		expect(
			resolveBasemap({ theme: 'light', hideBasemapLabels: true, cartoApiKey: ' abc ' })
		).toEqual(keyed);
		expect(cartoStyleUrl(CARTO_STYLES.dark, ' abc ')).toBe(`${CARTO_STYLES.dark}?key=abc`);
		expect(resolveTellegenUiConfig({ cartoApiKey: ' abc ' }).cartoApiKey).toBe('abc');
	});

	it('lets explicit styles win, as custom providers, and null switch the fallback off', () => {
		const inline = { version: 8 as const, sources: {}, layers: [] };
		expect(resolveBasemap({ theme: 'dark', hideBasemapLabels: true })).toEqual(
			defaultBasemap('dark')
		);
		expect(resolveBasemap({ mapStyle: inline, theme: 'light', hideBasemapLabels: false })).toEqual({
			primary: inline,
			fallback: CARTO_STYLES.light,
			hideLabels: false,
			primaryProvider: 'custom',
			fallbackProvider: 'carto'
		});
		expect(
			resolveBasemap({
				mapStyle: 'https://a.example/s.json',
				fallbackMapStyle: null,
				theme: 'light',
				hideBasemapLabels: true
			})
		).toEqual({
			primary: 'https://a.example/s.json',
			fallback: null,
			hideLabels: true,
			primaryProvider: 'custom',
			fallbackProvider: null
		});
		expect(
			resolveBasemap({
				fallbackMapStyle: 'https://b.example/s.json',
				theme: 'light',
				hideBasemapLabels: true,
				cartoApiKey: 'k'
			})
		).toEqual({
			primary: `${CARTO_STYLES['light-nolabels']}?key=k`,
			fallback: 'https://b.example/s.json',
			hideLabels: true,
			primaryProvider: 'carto',
			fallbackProvider: 'custom'
		});
	});

	it('names a host-supplied style custom even when CARTO or OpenFreeMap serves it', () => {
		const resolved = resolveBasemap({
			mapStyle: OPENFREEMAP_STYLES.dark,
			fallbackMapStyle: CARTO_STYLES.dark,
			theme: 'dark',
			hideBasemapLabels: true
		});
		expect(resolved.primaryProvider).toBe('custom');
		expect(resolved.fallbackProvider).toBe('custom');
	});
});

// Trimmed copies of the styles served on 2026-10-23: each keeps its sources and
// every layer's id, type, source, source-layer and zoom range, plus the paint
// of the background, water and land layers.
const fixture = (name: string) =>
	JSON.parse(
		readFileSync(new URL(`./fixtures/${name}.json`, import.meta.url), 'utf8')
	) as StyleSpecification;
const ofmDark = fixture('openfreemap-dark');
const ofmPositron = fixture('openfreemap-positron');
const cartoDarkMatter = fixture('carto-dark-matter');
const sourceLayer = (style: StyleSpecification, id: string) => {
	const layer = style.layers.find((l) => l.id === id)!;
	return 'source-layer' in layer ? layer['source-layer'] : undefined;
};

describe('OpenFreeMap tuning', () => {
	it('recognises OpenFreeMap styles by their tile source, not their URL', () => {
		expect(isOpenFreeMapStyle(ofmDark)).toBe(true);
		expect(isOpenFreeMapStyle(ofmPositron)).toBe(true);
		expect(isOpenFreeMapStyle(cartoDarkMatter)).toBe(false);
		expect(isOpenFreeMapStyle({ version: 8, sources: {}, layers: [] })).toBe(false);
	});

	it('starts every road line at z8, keeping its upper zoom', () => {
		for (const [style, theme] of [
			[ofmDark, 'dark'],
			[ofmPositron, 'light']
		] as const) {
			const roads = style.layers.filter(
				(l) => l.type === 'line' && 'source-layer' in l && l['source-layer'] === 'transportation'
			);
			const lowRoads = roads.filter((l) => (l.minzoom ?? 0) < 8);
			expect(lowRoads.length).toBeGreaterThan(0);
			const zoomEdits = openFreeMapEdits(style, theme).filter((e) => e.kind === 'zoom-range');
			expect(zoomEdits.map((e) => e.layer).sort()).toEqual(lowRoads.map((l) => l.id).sort());
			for (const edit of zoomEdits) {
				const layer = style.layers.find((l) => l.id === edit.layer)!;
				expect(edit).toEqual({
					kind: 'zoom-range',
					layer: layer.id,
					minzoom: 8,
					maxzoom: layer.maxzoom ?? 24
				});
			}
		}
	});

	it('changes no colours in the light theme', () => {
		expect(openFreeMapEdits(ofmPositron, 'light').every((e) => e.kind === 'zoom-range')).toBe(true);
		expect(openFreeMapEdits(ofmDark, 'light').every((e) => e.kind === 'zoom-range')).toBe(true);
	});

	it('in the dark theme, gives water Dark Matter colours and lays land on one tone', () => {
		const paints = openFreeMapEdits(ofmDark, 'dark').filter((e) => e.kind === 'paint');
		const byLayer = new Map(paints.map((e) => [e.layer, e]));
		const water = ofmDark.layers.filter((l) => sourceLayer(ofmDark, l.id) === 'water');
		const waterways = ofmDark.layers.filter(
			(l) => sourceLayer(ofmDark, l.id) === 'waterway' && l.type === 'line'
		);
		expect(water.length).toBeGreaterThan(0);
		for (const layer of water.filter((l) => l.type === 'fill')) {
			expect(byLayer.get(layer.id)).toMatchObject({ property: 'fill-color', value: '#2c353c' });
		}
		for (const layer of waterways) {
			expect(byLayer.get(layer.id)).toMatchObject({ property: 'line-color', value: '#3f5a6d' });
		}
		// Land fills take the style's own background, except a patterned fill,
		// whose colour MapLibre ignores.
		const background = ofmDark.layers.find((l) => l.type === 'background')!;
		const land = (background.paint as Record<string, unknown>)['background-color'];
		const landFills = ofmDark.layers.filter(
			(l) =>
				l.type === 'fill' &&
				['landuse', 'landcover', 'park'].includes(sourceLayer(ofmDark, l.id) ?? '')
		);
		expect(landFills.length).toBeGreaterThan(0);
		for (const layer of landFills) {
			const patterned = (layer.paint as Record<string, unknown>)['fill-pattern'] !== undefined;
			expect(byLayer.get(layer.id)).toEqual(
				patterned
					? undefined
					: { kind: 'paint', layer: layer.id, property: 'fill-color', value: land }
			);
		}
		// Nothing else is repainted.
		const expected = new Set(
			[...water.filter((l) => l.type === 'fill'), ...waterways, ...landFills].map((l) => l.id)
		);
		expect(paints.every((e) => expected.has(e.layer))).toBe(true);
	});

	it('follows source-layer, not layer ids', () => {
		const renamed: StyleSpecification = {
			...ofmDark,
			layers: ofmDark.layers.map((l) => ({ ...l, id: `renamed-${l.id}` }))
		};
		const strip = (id: string) => id.replace(/^renamed-/, '');
		expect(openFreeMapEdits(renamed, 'dark').map((e) => ({ ...e, layer: strip(e.layer) }))).toEqual(
			openFreeMapEdits(ofmDark, 'dark')
		);
	});
});

describe('attachBasemapFallback', () => {
	let consoleError: ReturnType<typeof vi.spyOn>;
	beforeEach(() => {
		consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
	});
	afterEach(() => {
		consoleError.mockRestore();
	});

	it('rule 1: a style fetch failure before style.load switches to the fallback once', () => {
		const map = new FakeMap();
		const { onFallback, onFailure } = attach(map);
		map.emit('error', { error: ajaxError(0, 'Failed to fetch') });
		expect(map.styles).toEqual([{ style: spec.fallback, options: { diff: false } }]);
		expect(onFallback).toHaveBeenCalledTimes(1);
		expect(onFallback.mock.calls[0][0]).toEqual({
			kind: 'style-load',
			message: 'Failed to fetch',
			status: 0,
			from: 'openfreemap',
			to: 'carto'
		});
		expect(onFailure).not.toHaveBeenCalled();
		expect(consoleError).not.toHaveBeenCalled();
	});

	it('rule 1 reports the HTTP status of the style response', () => {
		const map = new FakeMap();
		const { onFallback } = attach(map);
		map.emit('error', { error: ajaxError(503, 'Service Unavailable') });
		expect(onFallback.mock.calls[0][0]).toMatchObject({ kind: 'style-load', status: 503 });
	});

	it('once only: a failing fallback style reports failure instead of switching again', () => {
		const map = new FakeMap();
		const { onFallback, onFailure } = attach(map);
		map.emit('error', { error: ajaxError(0, 'Failed to fetch') });
		map.emit('error', { error: ajaxError(0, 'Failed to fetch') });
		map.emit('error', { error: ajaxError(500, 'Internal Server Error') });
		expect(map.styles).toHaveLength(1);
		expect(onFallback).toHaveBeenCalledTimes(1);
		expect(onFailure).toHaveBeenCalledTimes(1);
		expect(onFailure.mock.calls[0][0]).toMatchObject({
			kind: 'style-load',
			status: 0,
			from: 'carto',
			to: null
		});
	});

	it('a style error after style.load is passed through, not treated as a fetch failure', () => {
		const map = new FakeMap();
		const { onFallback, onFailure } = attach(map);
		map.emit('style.load');
		const error = new Error('An image named "x" already exists.');
		map.emit('error', { error });
		expect(map.styles).toHaveLength(0);
		expect(onFallback).not.toHaveBeenCalled();
		expect(onFailure).not.toHaveBeenCalled();
		expect(consoleError).toHaveBeenCalledWith(error);
	});

	it('seeds the loaded state from the map when attached late', () => {
		const map = new FakeMap();
		map.loaded = true;
		const { onFallback } = attach(map);
		map.emit('error', { error: new Error('late style error') });
		expect(onFallback).not.toHaveBeenCalled();
		expect(map.styles).toHaveLength(0);
	});

	it('rule 2: a forbidden, unauthorized or rate-limited source switches', () => {
		for (const [status, text] of [
			[401, 'Unauthorized'],
			[403, 'Forbidden'],
			[429, 'Too Many Requests']
		] as const) {
			const map = new FakeMap();
			const { onFallback } = attach(map);
			map.emit('style.load');
			map.emit('error', { sourceId: 'carto', tile: {}, error: ajaxError(status, text) });
			expect(map.styles).toEqual([{ style: spec.fallback, options: { diff: false } }]);
			expect(onFallback.mock.calls[0][0]).toEqual({
				kind: 'source-forbidden',
				message: text,
				status,
				sourceId: 'carto',
				from: 'openfreemap',
				to: 'carto'
			});
		}
	});

	it('rule 2 also matches an API key message without a status', () => {
		const map = new FakeMap();
		const { onFallback } = attach(map);
		map.emit('style.load');
		map.emit('error', { sourceId: 'carto', error: new Error('Missing API key') });
		expect(onFallback.mock.calls[0][0]).toEqual({
			kind: 'source-forbidden',
			message: 'Missing API key',
			status: 0,
			sourceId: 'carto',
			from: 'openfreemap',
			to: 'carto'
		});
	});

	it('rule 3: more than six tile fetch failures with no tile arriving switch', () => {
		const map = new FakeMap();
		const { onFallback } = attach(map);
		map.emit('style.load');
		for (let i = 0; i < 6; i++) {
			map.emit('error', {
				sourceId: 'openmaptiles',
				tile: {},
				error: ajaxError(0, 'Failed to fetch')
			});
		}
		expect(map.styles).toHaveLength(0);
		expect(consoleError).toHaveBeenCalledTimes(6);
		map.emit('error', {
			sourceId: 'openmaptiles',
			tile: {},
			error: ajaxError(0, 'Failed to fetch')
		});
		expect(map.styles).toHaveLength(1);
		expect(onFallback.mock.calls[0][0]).toEqual({
			kind: 'tiles-unavailable',
			message: 'Failed to fetch',
			sourceId: 'openmaptiles',
			from: 'openfreemap',
			to: 'carto'
		});
	});

	it('rule 3 stands down when a tile arrives inside the window', () => {
		const map = new FakeMap();
		const { onFallback } = attach(map);
		map.emit('style.load');
		for (let i = 0; i < 4; i++) {
			map.emit('error', {
				sourceId: 'openmaptiles',
				tile: {},
				error: ajaxError(0, 'Failed to fetch')
			});
		}
		map.emit('sourcedata', { sourceId: 'openmaptiles', dataType: 'source', tile: {} });
		for (let i = 0; i < 6; i++) {
			map.emit('error', {
				sourceId: 'openmaptiles',
				tile: {},
				error: ajaxError(0, 'Failed to fetch')
			});
		}
		expect(map.styles).toHaveLength(0);
		expect(onFallback).not.toHaveBeenCalled();
	});

	it('rule 3 forgets failures older than the ten second window', () => {
		let t = 0;
		const map = new FakeMap();
		const { onFallback } = attach(map, {}, () => t);
		map.emit('style.load');
		for (let i = 0; i < 6; i++) {
			map.emit('error', {
				sourceId: 'openmaptiles',
				tile: {},
				error: ajaxError(0, 'Failed to fetch')
			});
		}
		t = 10_001;
		map.emit('error', {
			sourceId: 'openmaptiles',
			tile: {},
			error: ajaxError(0, 'Failed to fetch')
		});
		expect(onFallback).not.toHaveBeenCalled();
		for (let i = 0; i < 6; i++) {
			map.emit('error', {
				sourceId: 'openmaptiles',
				tile: {},
				error: ajaxError(0, 'Failed to fetch')
			});
		}
		expect(onFallback).toHaveBeenCalledTimes(1);
	});

	it('rule 3 switches when tile servers answer 5xx and no tile arrives', () => {
		const map = new FakeMap();
		const { onFallback } = attach(map);
		map.emit('style.load');
		for (let i = 0; i < 10; i++) {
			map.emit('error', {
				sourceId: 'openmaptiles',
				tile: {},
				error: ajaxError(503, 'Service Unavailable')
			});
		}
		expect(onFallback).toHaveBeenCalledTimes(1);
		expect(onFallback.mock.calls[0][0]).toMatchObject({ kind: 'tiles-unavailable' });
	});

	it('rule 3 ignores 500s among tiles that arrive', () => {
		const map = new FakeMap();
		const { onFallback } = attach(map);
		map.emit('style.load');
		map.emit('error', {
			sourceId: 'openmaptiles',
			tile: {},
			error: ajaxError(500, 'Internal Server Error')
		});
		map.emit('sourcedata', { sourceId: 'openmaptiles', dataType: 'source', tile: {} });
		for (let i = 0; i < 9; i++) {
			map.emit('error', {
				sourceId: 'openmaptiles',
				tile: {},
				error: ajaxError(500, 'Internal Server Error')
			});
		}
		expect(onFallback).not.toHaveBeenCalled();
		expect(consoleError).toHaveBeenCalledTimes(10);
	});

	it('rule 4: a WebGL error reports failure without touching the style', () => {
		const map = new FakeMap();
		const { onFallback, onFailure } = attach(map);
		map.emit('error', { error: new Error('Failed to initialize WebGL') });
		expect(map.styles).toHaveLength(0);
		expect(onFallback).not.toHaveBeenCalled();
		expect(onFailure.mock.calls[0][0]).toEqual({
			kind: 'webgl',
			message: 'Failed to initialize WebGL',
			from: 'openfreemap',
			to: null
		});
		expect(consoleError).not.toHaveBeenCalled();
	});

	it('rule 5: anything else is logged the way MapLibre would have logged it', () => {
		const map = new FakeMap();
		const { onFallback, onFailure } = attach(map);
		map.emit('style.load');
		const error = new Error('Unexpected glyph error');
		map.emit('error', { sourceId: 'openmaptiles', error });
		map.emit('error', {});
		expect(onFallback).not.toHaveBeenCalled();
		expect(onFailure).not.toHaveBeenCalled();
		expect(consoleError).toHaveBeenNthCalledWith(1, error);
		expect(consoleError).toHaveBeenNthCalledWith(2, {});
	});

	it('reports failure directly when no fallback is configured', () => {
		const map = new FakeMap();
		const { onFallback, onFailure } = attach(map, { fallback: null });
		map.emit('error', { error: ajaxError(0, 'Failed to fetch') });
		expect(map.styles).toHaveLength(0);
		expect(onFallback).not.toHaveBeenCalled();
		expect(onFailure.mock.calls[0][0]).toMatchObject({
			kind: 'style-load',
			from: 'openfreemap',
			to: null
		});
	});

	it('names both providers custom when the spec does not name them', () => {
		const map = new FakeMap();
		const onFallback = vi.fn<(r: BasemapFallbackReason) => void>();
		const onFailure = vi.fn<(r: BasemapFallbackReason) => void>();
		attachBasemapFallback(
			map,
			{ primary: spec.primary, fallback: spec.fallback, hideLabels: true },
			{ onFallback, onFailure }
		);
		map.emit('error', { error: ajaxError(0, 'Failed to fetch') });
		map.emit('error', { error: ajaxError(0, 'Failed to fetch') });
		expect(onFallback.mock.calls[0][0]).toMatchObject({ from: 'custom', to: 'custom' });
		expect(onFailure.mock.calls[0][0]).toMatchObject({ from: 'custom', to: null });
	});

	it('names the providers of a keyed default spec: CARTO first, then OpenFreeMap', () => {
		const map = new FakeMap();
		const onFallback = vi.fn<(r: BasemapFallbackReason) => void>();
		const onFailure = vi.fn<(r: BasemapFallbackReason) => void>();
		attachBasemapFallback(map, defaultBasemap('dark', { cartoApiKey: 'k' }), {
			onFallback,
			onFailure
		});
		map.emit('style.load');
		map.emit('error', { sourceId: 'carto', tile: {}, error: ajaxError(401, 'Unauthorized') });
		expect(onFallback.mock.calls[0][0]).toMatchObject({ from: 'carto', to: 'openfreemap' });
		map.emit('error', { error: ajaxError(0, 'Failed to fetch') });
		expect(onFailure.mock.calls[0][0]).toMatchObject({
			kind: 'style-load',
			from: 'openfreemap',
			to: null
		});
	});

	it('after the switch, forbidden tiles from the fallback report failure once', () => {
		const map = new FakeMap();
		const { onFallback, onFailure } = attach(map);
		map.emit('error', { error: ajaxError(0, 'Failed to fetch') });
		map.emit('style.load');
		map.emit('error', { sourceId: 'carto', tile: {}, error: ajaxError(403, 'Forbidden') });
		map.emit('error', { sourceId: 'carto', tile: {}, error: ajaxError(403, 'Forbidden') });
		expect(onFallback).toHaveBeenCalledTimes(1);
		expect(onFailure).toHaveBeenCalledTimes(1);
		expect(onFailure.mock.calls[0][0]).toMatchObject({
			kind: 'source-forbidden',
			status: 403,
			from: 'carto',
			to: null
		});
		expect(map.styles).toHaveLength(1);
	});

	it('detach removes every listener', () => {
		const map = new FakeMap();
		const { onFallback, detach } = attach(map);
		expect(map.listenerCount()).toBe(3);
		detach();
		expect(map.listenerCount()).toBe(0);
		map.emit('error', { error: ajaxError(0, 'Failed to fetch') });
		expect(onFallback).not.toHaveBeenCalled();
		expect(map.styles).toHaveLength(0);
	});
});
