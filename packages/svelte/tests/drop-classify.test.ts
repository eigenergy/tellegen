import { describe, expect, it } from 'vitest';
import {
	classifyJson,
	distExtensionFormat,
	isStudyPackageText, isGeoFileName } from '../src/lib/drop-classify.js';

/** A `.pio.json` envelope as powerio 0.9 writes it: `model_kind` beside `model`.
 * The `schema` URL field this helper used to add left the document in 0.8.0. */
const pkg = (kind: unknown, fields: Record<string, unknown> = {}) =>
	JSON.stringify({ model_kind: kind, model: { kind }, powerio_version: '0.9.0', ...fields });

describe('distExtensionFormat', () => {
	it('routes .dss by extension, case-insensitively', () => {
		expect(distExtensionFormat('feeder.dss')).toBe('dss');
		expect(distExtensionFormat('FEEDER.DSS')).toBe('dss');
		expect(distExtensionFormat('Master.DsS')).toBe('dss');
	});

	it('returns null for JSON and balanced case extensions (content-sniffed or name-routed elsewhere)', () => {
		for (const name of ['case.json', 'case.m', 'grid.raw', 'grid.aux', 'note.txt', 'noext']) {
			expect(distExtensionFormat(name)).toBeNull();
		}
	});
});

describe('classifyJson package envelopes', () => {
	it('splits a package by its authoritative model_kind', () => {
		expect(classifyJson(pkg('balanced'))).toBe('balanced-package');
		expect(classifyJson(pkg('multiconductor'))).toBe('multiconductor-package');
	});

	it('recognizes a package with no schema URL field, which powerio stopped writing in 0.8.0', () => {
		const saved = JSON.stringify({
			powerio_version: '0.9.0',
			producer: { tool: 'powerio', version: '0.9.0' },
			model_kind: 'balanced',
			model: { kind: 'balanced', balanced_network: { base_mva: 100.0, buses: [] } }
		});
		expect(classifyJson(saved)).toBe('balanced-package');
	});

	it('needs both markers, so a case document carrying one key name is not a package', () => {
		expect(classifyJson(JSON.stringify({ model_kind: 'balanced' }))).not.toBe('balanced-package');
		expect(classifyJson(JSON.stringify({ model: { kind: 'balanced' } }))).not.toBe(
			'balanced-package'
		);
		expect(classifyJson(JSON.stringify({ model_kind: 'something else', model: {} }))).not.toBe(
			'balanced-package'
		);
	});
});

describe('classifyJson model JSON', () => {
	it('routes powerio model JSON to its own path, not to a distribution reader', () => {
		expect(classifyJson(JSON.stringify({ buses: [], branches: [] }))).toBe('model-json');
		expect(classifyJson(JSON.stringify({ base_mva: 100, buses: [], generators: [] }))).toBe(
			'model-json'
		);
	});

	it('keys on the plural table name, which the case formats do not use', () => {
		// PowerModels writes `bus`, singular, and is a case format.
		expect(classifyJson(JSON.stringify({ bus: {}, branch: {} }))).toBe('bmopf');
		// `buses` alone, with no other network key, is not enough.
		expect(classifyJson(JSON.stringify({ buses: [] }))).toBe('bmopf');
	});
});

describe('classifyJson distribution documents', () => {
	it('routes a PMD ENGINEERING document by its data_model marker', () => {
		expect(classifyJson(JSON.stringify({ data_model: 'ENGINEERING', bus: {} }))).toBe('pmd');
	});

	it('routes any other JSON object to BMOPF', () => {
		expect(classifyJson(JSON.stringify({ bus: {}, line: {} }))).toBe('bmopf');
		// A nested data_model must not be mistaken for the top-level PMD marker.
		expect(classifyJson(JSON.stringify({ bus: { b1: { data_model: {} } } }))).toBe('bmopf');
		// A `data_model`-named value that is not a top-level key stays BMOPF.
		expect(classifyJson(JSON.stringify({ name: 'data_model' }))).toBe('bmopf');
	});

	it('leaves a GeoJSON FeatureCollection unrouted so it reaches the geo sidecar path', () => {
		expect(classifyJson(JSON.stringify({ type: 'FeatureCollection', features: [] }))).toBe(
			'not-json'
		);
	});
});

describe('classifyJson totality', () => {
	it('classifies non-JSON, arrays, scalars, and truncated input as not-json', () => {
		for (const bad of ['', '   ', '{', ']', 'not json', 'null', '[]', '42', '"data_model"']) {
			expect(classifyJson(bad)).toBe('not-json');
		}
	});
});

describe('isStudyPackageText', () => {
	it('is true only for a balanced package envelope', () => {
		expect(isStudyPackageText(pkg('balanced'))).toBe(true);
		expect(isStudyPackageText(pkg('multiconductor'))).toBe(false);
		expect(isStudyPackageText(JSON.stringify({ buses: [], branches: [] }))).toBe(false);
		expect(isStudyPackageText(JSON.stringify({ schema: 'https://example.com/other' }))).toBe(
			false
		);
	});

	it('returns false for non-JSON or truncated input rather than throwing', () => {
		for (const bad of ['', '   ', '{', 'not json', 'null', '[]', '42']) {
			expect(isStudyPackageText(bad)).toBe(false);
		}
	});
});

describe('isGeoFileName', () => {
	it('routes .csv, .json, and .geojson to the geo sidecar path by extension', () => {
		expect(isGeoFileName('coords.csv')).toBe(true);
		expect(isGeoFileName('coords.JSON')).toBe(true);
		expect(isGeoFileName('coords.geojson')).toBe(true);
		expect(isGeoFileName('case14.m')).toBe(false);
		expect(isGeoFileName('case.raw')).toBe(false);
		expect(isGeoFileName('feeder.dss')).toBe(false);
		expect(isGeoFileName('diagram.pwd')).toBe(false);
		expect(isGeoFileName('csv')).toBe(false);
	});
});
