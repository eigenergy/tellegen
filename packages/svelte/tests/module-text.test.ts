import { afterEach, describe, expect, it, vi } from 'vitest';
import { readBusKinds } from '../src/lib/bus-kinds.js';
import { readModuleText } from '../src/lib/module-text.js';
import { recordedCalculationSettings } from '../src/lib/settings.svelte.js';

// A generated module carries both the bus kinds and recorded settings; the
// viewer reads each when it opens one, and must parse the text once (#261).
const generated = JSON.stringify({
	schema: 'pio-ir',
	version: 2,
	value: {
		type: 'powerio.BalancedNetwork',
		data: {
			buses: [
				{ id: 1, extras: { tellegen_synth: { kind: 'substation' } } },
				{ id: 2, extras: { tellegen_synth: { kind: 'tap' } } }
			]
		}
	},
	extensions: {
		'tellegen.calculation_settings': {
			version: 1,
			calculation: 'dcopf',
			overrides: { thermal: 'rated' }
		}
	}
});

describe('readModuleText', () => {
	afterEach(() => vi.restoreAllMocks());

	it('parses a module once for its settings and bus kinds', () => {
		const parse = vi.spyOn(JSON, 'parse');
		// Text no earlier test read, so nothing is cached for it yet.
		const text = generated + ' ';
		expect(recordedCalculationSettings(text)?.overrides).toEqual({
			thermal: 'rated'
		});
		expect(recordedCalculationSettings(text)?.calculation).toBe('dcopf');
		expect([...readBusKinds(text)]).toEqual([
			[1, 'substation'],
			[2, 'tap']
		]);
		expect(parse).toHaveBeenCalledTimes(1);
	});

	it('hands each settings reader its own copy', () => {
		const first = recordedCalculationSettings(generated)!;
		first.overrides.thermal = 'changed';
		expect(recordedCalculationSettings(generated)?.overrides).toEqual({
			thermal: 'rated'
		});
	});

	it('does not parse a module that names neither key', () => {
		const parse = vi.spyOn(JSON, 'parse');
		const plain = JSON.stringify({ value: { data: { buses: [{ id: 1 }] } } });
		expect(readModuleText(plain).busKinds.size).toBe(0);
		expect(recordedCalculationSettings(plain)).toBeNull();
		expect(parse).not.toHaveBeenCalled();
	});

	it('reads nothing from text that does not parse', () => {
		const broken = '{"tellegen_synth": ';
		expect(readModuleText(broken).busKinds.size).toBe(0);
		expect(readModuleText(broken).recordedSettings).toBeUndefined();
		expect(recordedCalculationSettings(broken)).toBeNull();
	});
});
