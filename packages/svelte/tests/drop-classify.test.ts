import { describe, expect, it } from 'vitest';
import { distExtensionFormat, isGeoFileName } from '../src/lib/drop-classify.js';

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
