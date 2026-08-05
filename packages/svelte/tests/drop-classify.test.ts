import { describe, expect, it } from 'vitest';
import {
	PIO_PACKAGE_SCHEMA_PREFIX,
	classifyJson,
	distExtensionFormat,
	isStudyPackageText, isGeoFileName } from '../src/lib/drop-classify.js';

/** The envelope spelling powerio 0.7.x wrote: a `schema` URL plus a version. */
const legacyPkg = (fields: Record<string, unknown>) =>
	JSON.stringify({
		schema: `${PIO_PACKAGE_SCHEMA_PREFIX}/0.1`,
		schema_version: '0.1.1',
		...fields
	});

/** The envelope spelling that follows it: `schema_version` alone, no `schema`. */
const versionedPkg = (fields: Record<string, unknown>) =>
	JSON.stringify({ schema_version: '0.2.0', ...fields });

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

describe.each([
	['legacy schema URL', legacyPkg],
	['schema_version only', versionedPkg]
])('classifyJson package envelopes (%s)', (_label, pkg) => {
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

	it('routes a saved study to the restore path', () => {
		expect(isStudyPackageText(pkg({ model_kind: 'balanced' }))).toBe(true);
		expect(isStudyPackageText(pkg({ model_kind: 'multiconductor' }))).toBe(false);
	});
});

describe('classifyJson package envelope recognition', () => {
	it('treats a legacy package with no readable kind as balanced (the historical payload)', () => {
		expect(classifyJson(legacyPkg({}))).toBe('balanced-package');
	});

	it('does not take schema_version alone as a package envelope', () => {
		// A case document could carry a `schema_version`; the envelope is the
		// pairing with a model kind. Without one this stays a distribution
		// document, which is where the precise parse error comes from.
		expect(classifyJson(versionedPkg({}))).toBe('bmopf');
		expect(classifyJson(versionedPkg({ data_model: 'ENGINEERING' }))).toBe('pmd');
	});

	it('ignores a non-string schema_version', () => {
		expect(classifyJson(JSON.stringify({ schema_version: 2, model_kind: 'balanced' }))).toBe(
			'bmopf'
		);
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
		// The per-spelling suites above cover the balanced/multiconductor split
		// for both envelope forms; this pins what is *not* a study package.
		expect(isStudyPackageText(legacyPkg({ model_kind: 'balanced' }))).toBe(true);
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
