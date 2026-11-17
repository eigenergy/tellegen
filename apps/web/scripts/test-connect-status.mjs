import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
	CONNECT_STATUSES,
	connectStatus,
	unavailableTools
} from '../src/lib/webmcp/connect-status.ts';

const state = (overrides) => ({
	supported: false,
	registrationError: null,
	browser: null,
	...overrides
});

test('every status has one row, and each case maps to it', () => {
	assert.equal(new Set(CONNECT_STATUSES.map((s) => s.id)).size, CONNECT_STATUSES.length);
	assert.equal(connectStatus(state({})).id, 'loading');
	assert.equal(connectStatus(state({ browser: { kind: 'ready', mobile: false } })).id, 'loading');
	assert.equal(connectStatus(state({ supported: true })).label, 'Connected');
	assert.equal(connectStatus(state({ browser: { kind: 'mobile' } })).id, 'unsupported');
	assert.equal(
		connectStatus(state({ browser: { kind: 'other', browser: 'Firefox' } })).label,
		'Not available in this browser'
	);
});

test('a registration error wins over everything else', () => {
	const status = connectStatus(state({ supported: true, registrationError: 'boom' }));
	assert.equal(status.label, 'Tools failed to load');
});

test('an old browser is named with the version it needs', () => {
	const old = { kind: 'update', brand: 'Edge', version: 149, minVersion: 150 };
	assert.equal(connectStatus(state({ browser: old })).label, 'Needs Edge 150+');
	const legacy = { kind: 'legacy', brand: 'Chromium', version: null, minVersion: 149 };
	assert.equal(connectStatus(state({ browser: legacy })).label, 'Needs this browser 149+');
});

test('a browser with WebMCP off gets the flag address', () => {
	const flagUrl = 'chrome://flags/#enable-webmcp-testing';
	const status = connectStatus(
		state({ browser: { kind: 'flag', brand: 'Chrome', version: 157, flagUrl } })
	);
	assert.deepEqual(
		{ id: status.id, label: status.label, flagUrl: status.flagUrl },
		{ id: 'off', label: 'Off in this browser', flagUrl }
	);
});

test('tools that are not registered are listed with a reason', () => {
	assert.deepEqual(unavailableTools(['simulate_dynamics', 'propose_capacity_plan']), []);
	assert.deepEqual(
		unavailableTools(['inspect_case']).map((tool) => tool.name),
		['simulate_dynamics', 'propose_capacity_plan']
	);
});
