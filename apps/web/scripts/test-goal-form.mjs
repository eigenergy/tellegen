import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
	buildGoal,
	defaultGoalForm,
	resolveEquipment,
	candidateBounds
} from '../src/lib/studies/goal-form.ts';

const network = {
	id: 'test',
	name: 'Test network',
	base_mva: 100,
	synthetic_coords: false,
	buses: Array.from({ length: 35 }, (_, index) => ({
		id: index + 1,
		uid: `bus:${index + 1}`,
		name: `Station ${index + 1}`,
		area: index < 10 ? 'North' : 'South',
		lon: -80,
		lat: 34,
		demand_mw: index + 1,
		gen_mw: 0
	})),
	branches: Array.from({ length: 34 }, (_, index) => ({
		id: index + 1,
		uid: `line:${index + 1}`,
		from: index + 1,
		to: index + 2,
		rate_mw: 100,
		status: 1,
		path: []
	}))
};

test('whole-network planning keeps every eligible bus and line', () => {
	const form = defaultGoalForm('dcopf');
	const goal = buildGoal(network, form, 'dcopf');
	assert.equal(goal.objective.weights.length, 35);
	assert.equal(goal.decisions.variables.length, 34);
	assert.equal(goal.objective.weights.at(-1).element, 'bus:35');
	assert.ok(
		Math.abs(goal.objective.weights.reduce((sum, item) => sum + item.weight, 0) - 1) < 1e-12
	);
});

test('changing controls changes the submitted model immediately', () => {
	const form = defaultGoalForm('dcopf');
	const first = buildGoal(network, form, 'dcopf');
	form.budget = 40;
	form.increment = 2;
	form.weighting = 'equal';
	form.cardinality = 4;
	const next = buildGoal(network, form, 'dcopf');
	assert.equal(next.decisions.total_budget, 40);
	assert.equal(next.decisions.variables.at(-1).upper, 40);
	assert.equal(next.decisions.variables.at(-1).increment, 2);
	assert.equal(next.decisions.max_changed_elements, 4);
	assert.equal(next.objective.weights[0].weight, 1 / 35);
	assert.notEqual(first.objective.weights[0].weight, next.objective.weights[0].weight);
});

test('explicit selection uses stable identities independently for objective and decisions', () => {
	const form = defaultGoalForm('dcopf');
	form.scope = 'selected';
	form.buses = ['bus:2', 'bus:35'];
	form.candidates = ['line:34'];
	const goal = buildGoal(network, form, 'dcopf');
	assert.deepEqual(
		goal.objective.weights.map((item) => item.element),
		['bus:2', 'bus:35']
	);
	assert.deepEqual(
		goal.decisions.variables.map((item) => item.element),
		['line:34']
	);
	assert.equal(goal.decisions.max_changed_elements, 1);
});

test('area includes connected lines and weights only its objective buses', () => {
	const form = defaultGoalForm('dcopf');
	form.scope = 'area';
	form.area = 'North';
	const goal = buildGoal(network, form, 'dcopf');
	assert.equal(goal.objective.weights.length, 10);
	assert.equal(goal.decisions.variables.length, 10);
	assert.equal(goal.decisions.variables.at(-1).element, 'line:10');
});

test('redistribution includes receiving buses with zero initial demand', () => {
	const withReceiver = structuredClone(network);
	withReceiver.buses[34].demand_mw = 0;
	const form = defaultGoalForm('dcopf');
	form.intervention = 'redistribution';
	form.increment = 1;
	const goal = buildGoal(withReceiver, form, 'dcopf');
	assert.equal(goal.decisions.variables.length, 35);
	assert.equal(goal.decisions.variables.at(-1).lower, 0);
	assert.equal(goal.decisions.variables[0].lower, -1);
	assert.deepEqual(goal.decisions.demand, { kind: 'redistribution' });
});

test('custom bounds remain visible and cannot create negative demand', () => {
	const form = defaultGoalForm('dcopf');
	form.intervention = 'redistribution';
	form.bounds['bus:35'] = { upper: 8 };
	assert.equal(candidateBounds(network.buses[34], form).upper, 8);
	form.bounds['bus:1'] = { lower: -2 };
	assert.throws(() => buildGoal(network, form, 'dcopf'), /below zero/);
});

test('unsupported physics and incomplete selected equipment fail before execution', () => {
	assert.throws(
		() => buildGoal(network, defaultGoalForm('dcopf'), 'acpf'),
		/does not calculate LMPs/
	);
	const form = defaultGoalForm('dcopf');
	form.scope = 'selected';
	assert.throws(() => buildGoal(network, form, 'dcopf'), /at least one bus/);
	form.buses = ['bus:2'];
	assert.throws(() => buildGoal(network, form, 'dcopf'), /at least one line or bus/);
	assert.equal(resolveEquipment(network, form).candidates.length, 0);
});

test('AC power flow defaults to a voltage objective and demand changes', () => {
	const form = defaultGoalForm('acpf');
	const goal = buildGoal(network, form, 'acpf');
	assert.equal(goal.objective.kind, 'sum');
	assert.equal(goal.objective.terms.length, 35);
	assert.ok(
		goal.decisions.variables.every((variable) => variable.intervention === 'active_demand')
	);
});
