import { describe, expect, it } from 'vitest';
import {
	docsLink,
	economicsLead,
	operatingLead,
	transferLead
} from '../src/lib/diagram/transmission-explain.js';

const counts = {
	low: 0,
	high: 0,
	overload: 0,
	warning: 0,
	atLimit: 0,
	loadingUnknown: 0
};

describe('operating limits lead', () => {
	it('says plainly when nothing is wrong, and why DC checks loading only', () => {
		const lead = operatingLead({
			hasResult: true,
			voltageSolved: false,
			voltageAvailable: false,
			counts,
			warning: 90
		});
		expect(lead.tone).toBe('ok');
		expect(lead.text).toBe('Every rated line runs below 90% of its rating.');
		expect(lead.why).toContain('DC power flow treats every voltage as 1 pu');
	});

	it('leads with overloads, then voltages, and names lines without a rating', () => {
		const lead = operatingLead({
			hasResult: true,
			voltageSolved: true,
			voltageAvailable: true,
			counts: { ...counts, overload: 2, low: 1, loadingUnknown: 3 },
			warning: 90
		});
		expect(lead.tone).toBe('violation');
		expect(lead.text).toBe(
			'2 lines carry more than their rating. 1 bus is outside its voltage limits. 3 lines have no rating to check.'
		);
	});

	it('warns about lines near their rating without calling them overloaded', () => {
		const lead = operatingLead({
			hasResult: true,
			voltageSolved: true,
			voltageAvailable: true,
			counts: { ...counts, warning: 1, atLimit: 1 },
			warning: 80
		});
		expect(lead.tone).toBe('warning');
		expect(lead.text).toBe(
			'No line is over its rating, but 2 run at 80% or more of it. Every bus voltage is within its limits.'
		);
	});

	it('asks for a solve when there is no result', () => {
		const lead = operatingLead({
			hasResult: false,
			voltageSolved: true,
			voltageAvailable: false,
			counts,
			warning: 90
		});
		expect(lead.text).toBe('No result yet.');
	});
});

describe('OPF economics lead', () => {
	const base = { opf: true, ac: false, prices: [6.71, 6.71], binding: 0, thermalAvailable: true };

	it('explains a uniform price as the absence of binding limits', () => {
		const lead = economicsLead({ ...base, uniform: 6.71 });
		expect(lead.text).toContain('Every bus has the same price: 6.71');
		expect(lead.why).toContain('No line limit holds prices apart');
		expect(lead.why).toContain('prices separate on either side of it');
	});

	it('ties separated prices to the binding limits', () => {
		const lead = economicsLead({ ...base, prices: [5, 40], uniform: null, binding: 1 });
		expect(lead.text).toMatch(/^Prices range from 5 .* to 40 /);
		expect(lead.why).toMatch(
			/^1 line limit binds \(wide purple\): power cannot flow further across it/
		);
	});

	it('credits AC losses when no thermal limit binds', () => {
		const lead = economicsLead({ ...base, ac: true, prices: [20, 21], uniform: null });
		expect(lead.why).toContain('losses and voltage limits');
	});

	it('says where prices come from for a power flow', () => {
		const lead = economicsLead({ ...base, opf: false, prices: [], uniform: null });
		expect(lead.text).toBe('Prices come from an optimal power flow (OPF).');
		expect(lead.why).toContain('Choose DC OPF, AC OPF or SOCWR');
	});
});

describe('power transfers lead', () => {
	it('names the busiest line and how to read width and chevrons', () => {
		const lead = transferLead({
			hasResult: true,
			top: { id: 7, mw: '555 MW', direction: 'Bus 189 → bus 187' },
			scale: 500
		});
		expect(lead.text).toBe('The busiest line, branch 7, carries 555 MW (Bus 189 → bus 187).');
		expect(lead.why).toContain('up to 500 MW');
	});
});

describe('docs links', () => {
	it('points each view at its section of the transmission diagrams page', () => {
		expect(docsLink('https://eigenergy.github.io/tellegen/', 'economics')).toBe(
			'https://eigenergy.github.io/tellegen/transmission-diagrams.html#opf-economics'
		);
		expect(docsLink('https://example.org/docs', 'operating')).toBe(
			'https://example.org/docs/transmission-diagrams.html#results'
		);
	});
});
