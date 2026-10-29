import { writeFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
	buildSearchIndex,
	displayTree,
	graphInputs,
	queryTable,
	rankIssues,
	search,
	snapshotFromMcPf,
	snapshotSummary,
	tableCsv,
	tableModel
} from '../src/lib/diagram/index.js';
import { syntheticFeeder } from './diagram-fixtures.js';

/** Every step the Studies overview runs after a solve, plus the interactive ones. */
function overviewSteps(n: number) {
	const { graph, result } = syntheticFeeder(n);
	let inputs!: ReturnType<typeof graphInputs>;
	let snapshot!: ReturnType<typeof snapshotFromMcPf>;
	let issues!: ReturnType<typeof rankIssues>;
	let tree!: ReturnType<typeof displayTree>;
	let index!: ReturnType<typeof buildSearchIndex>;
	const steps: [string, () => unknown][] = [
		['graphInputs', () => (inputs = graphInputs(graph))],
		[
			'snapshotFromMcPf',
			() => (snapshot = snapshotFromMcPf(result, inputs.buses, inputs.branches))
		],
		['rankIssues', () => (issues = rankIssues(snapshot))],
		['snapshotSummary', () => snapshotSummary(snapshot, issues)],
		['displayTree', () => (tree = displayTree(graph))],
		['buildSearchIndex', () => (index = buildSearchIndex(inputs.entities))],
		['search', () => search(index, 'bus99')],
		[
			'branch table sorted by loading',
			() =>
				queryTable(tableModel(snapshot, inputs.names, 'branch'), {
					sortKey: 'loading',
					descending: true
				})
		],
		[
			'bus CSV, all rows',
			() => {
				const model = tableModel(snapshot, inputs.names, 'bus');
				return tableCsv(model, queryTable(model));
			}
		]
	];
	return { steps, get: () => ({ inputs, snapshot, issues, tree }) };
}

describe('diagram overview at scale', () => {
	it('stays correct on a 10,000-bus feeder', () => {
		const { steps, get } = overviewSteps(10_000);
		for (const [, run] of steps) run();
		const { snapshot, issues, tree } = get();
		expect(snapshot.buses).toHaveLength(10_000);
		expect(snapshot.buses.every((b) => b.unbalance?.definition === 'VUF')).toBe(true);
		expect(issues.length).toBeGreaterThan(0);
		expect(issues[0].severity).toBe('violation');
		expect(tree.roots).toEqual([0]);
		expect(Math.max(...tree.depth)).toBeGreaterThan(300);
	});

	// Timing is evidence, not a gate: set DIAGRAM_SCALE=100000 (and optionally DIAGRAM_SCALE_OUT).
	it.runIf(!!process.env.DIAGRAM_SCALE)(
		'records timings at DIAGRAM_SCALE buses',
		() => {
			const n = Number(process.env.DIAGRAM_SCALE);
			const runs = 5;
			const samples = new Map<string, number[]>();
			for (let r = 0; r < runs + 1; r++) {
				const { steps } = overviewSteps(n);
				for (const [name, run] of steps) {
					const t = performance.now();
					run();
					if (r > 0) samples.set(name, [...(samples.get(name) ?? []), performance.now() - t]);
				}
			}
			const median = (v: number[]) => [...v].sort((a, b) => a - b)[Math.floor(v.length / 2)];
			const rows = [...samples].map(([step, v]) => ({
				step,
				medianMs: Number(median(v).toFixed(2)),
				maxMs: Number(Math.max(...v).toFixed(2))
			}));
			// graphInputs depends only on the network, so it is not part of the per-solve cost.
			const afterSolve = ['snapshotFromMcPf', 'rankIssues', 'snapshotSummary'];
			const report = {
				buses: n,
				branches: n - 1,
				runs,
				warmupRuns: 1,
				node: process.version,
				platform: `${process.platform} ${process.arch}`,
				perSolveMedianMs: Number(
					rows
						.filter((r) => afterSolve.includes(r.step))
						.reduce((s, r) => s + r.medianMs, 0)
						.toFixed(2)
				),
				steps: rows
			};
			console.table(rows);
			console.log(`per-solve overview: ${report.perSolveMedianMs} ms`);
			if (process.env.DIAGRAM_SCALE_OUT)
				writeFileSync(process.env.DIAGRAM_SCALE_OUT, JSON.stringify(report, null, 2) + '\n');
		},
		300_000
	);
});
