import { describe, expect, it } from 'vitest';
import {
	PIO_PACKAGE_SCHEMA_PREFIX,
	classifyJson,
	distExtensionFormat,
	isStudyPackageText, isGeoFileName } from '../src/lib/drop-classify.js';

const pkg = (fields: Record<string, unknown>) =>
	JSON.stringify({ schema: `${PIO_PACKAGE_SCHEMA_PREFIX}/0.1`, ...fields });

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
		expect(classifyJson(pkg({ model_kind: 'balanced' }))).toBe('balanced-package');
		expect(classifyJson(pkg({ model_kind: 'multiconductor' }))).toBe('multiconductor-package');
	});

	it('falls back to the payload model.kind tag when model_kind is absent', () => {
		expect(classifyJson(pkg({ model: { kind: 'multiconductor' } }))).toBe(
			'multiconductor-package'
		);
		expect(classifyJson(pkg({ model: { kind: 'balanced' } }))).toBe('balanced-package');
	});

	it('treats a package with no readable kind as balanced (the historical payload)', () => {
		expect(classifyJson(pkg({}))).toBe('balanced-package');
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
		expect(isStudyPackageText(pkg({ model_kind: 'balanced' }))).toBe(true);
		expect(isStudyPackageText(pkg({ model_kind: 'multiconductor' }))).toBe(false);
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
