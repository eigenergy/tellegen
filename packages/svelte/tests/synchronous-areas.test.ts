import { render } from 'svelte/server';
import { describe, expect, it, vi } from 'vitest';
import { readModuleText } from '../src/lib/module-text.js';
import {
	areaSourceUrl,
	parseSynchronousAreas
} from '../src/lib/synchronous-areas.js';
import SynchronousAreas from '../src/lib/components/SynchronousAreas.svelte';

const plan = {
	areas: [
		{
			id: 4,
			name: 'Western',
			bus_ids: [101, 205],
			demand_mw: 600,
			basis: 'Approximate western division',
			source_url: 'https://example.org/areas'
		},
		{
			id: 9,
			name: 'Eastern',
			bus_ids: [804],
			demand_mw: 400,
			basis: 'Approximate eastern division',
			source_url: null
		}
	],
	links: [
		{
			from_bus: 205,
			to_bus: 804,
			from_area: 4,
			to_area: 9,
			rating_mw: 120,
			transfer_mw: 75
		}
	]
};
const moduleJson = JSON.stringify({
	extensions: { 'tellegen.synchronous_areas': plan }
});

describe('synchronous-area display', () => {
	it('reads area membership using source bus IDs and shares the module cache', () => {
		const parse = vi.spyOn(JSON, 'parse');
		const text = moduleJson + ' ';
		expect(readModuleText(text).synchronousAreas?.byBus.get(205)?.name).toBe(
			'Western'
		);
		expect(readModuleText(text).synchronousAreas?.byBus.get(804)?.name).toBe(
			'Eastern'
		);
		expect(readModuleText(text).synchronousAreas?.byBus.get(0)).toBeUndefined();
		expect(parse).toHaveBeenCalledTimes(1);
		parse.mockRestore();
	});
	it('shows areas, fixed planned transfer and distinct rating without claiming solved flow', () => {
		const html = render(SynchronousAreas, {
			props: { moduleJson, selectedBus: 804 }
		}).body;
		expect(html).toContain('Western');
		expect(html).toContain('Eastern');
		expect(html).toContain('75 MW');
		expect(html).toContain('120 MW');
		expect(html).toContain('planned transfer');
		expect(html).toContain('Selected bus’s area');
		expect(html).toContain('synthetic');
		expect(html).toContain('Approximate western division');
	});
	it('keeps zero and reverse planned transfers distinct', () => {
		const reverse = JSON.stringify({ extensions: { 'tellegen.synchronous_areas': {
			...plan, links: [{ ...plan.links[0], transfer_mw: -75 }]
		} } });
		expect(render(SynchronousAreas, { props: { moduleJson: reverse } }).body).toMatch(/Eastern\s+→\s+Western/);
		const zero = JSON.stringify({ extensions: { 'tellegen.synchronous_areas': {
			...plan, links: [{ ...plan.links[0], transfer_mw: 0 }]
		} } });
		expect(render(SynchronousAreas, { props: { moduleJson: zero } }).body).toContain('↔');
	});
	it('refuses conflicting membership, unknown endpoints and invalid transfer fields', () => {
		expect(
			parseSynchronousAreas({
				...plan,
				areas: [plan.areas[0], { ...plan.areas[1], bus_ids: [205] }]
			})
		).toBeNull();
		for (const change of [
			{ from_bus: 999 },
			{ transfer_mw: Infinity },
			{ rating_mw: -1 },
			{ transfer_mw: 121 }
		]) {
			expect(
				parseSynchronousAreas({
					...plan,
					links: [{ ...plan.links[0], ...change }]
				})
			).toBeNull();
		}
	});
	it('omits details for old cases and blocks unsafe source links', () => {
		expect(
			render(SynchronousAreas, { props: { moduleJson: '{}' } }).body
		).not.toContain('Synchronous areas');
		expect(areaSourceUrl('javascript:alert(1)')).toBeUndefined();
		expect(areaSourceUrl('https://example.org/source')).toBe(
			'https://example.org/source'
		);
	});
	it('labels incomplete planning records rather than displaying partial areas', () => {
		const invalid = JSON.stringify({
			extensions: { 'tellegen.synchronous_areas': { areas: plan.areas } }
		});
		expect(
			render(SynchronousAreas, { props: { moduleJson: invalid } }).body
		).toMatch(/records are\s+incomplete/);
	});
});
