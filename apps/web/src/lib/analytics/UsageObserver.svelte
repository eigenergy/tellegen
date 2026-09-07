<script lang="ts">
	import { untrack } from 'svelte';
	import { CaseState, getAppState, getController } from '@tellegen/svelte';
	import { trackUsage, type EventData } from './client.js';
	const app = getAppState();
	const ctrl = getController();
	const seenSolutions = new WeakMap<object, unknown>();
	const pendingSolves = new WeakMap<
		object,
		{ started: number; solution: unknown; errorRevision: number }
	>();
	const seenEdits = new WeakMap<object, { demand: unknown; capacity: unknown }>();
	let selection: object | null = null;
	let display: string | null = null;
	let importing: { started: number; count: number; errorRevision: number } | null = null;
	let sensitivity: { started: number; errorRevision: number } | null = null;
	function context(c: { id: string }): EventData {
		return c instanceof CaseState ? { source: 'demo', demo_case: c.id } : { source: 'local' };
	}
	$effect(() => {
		const current = app.studyView ?? ctrl.activeSolvable ?? app.activeMulti;
		if (current !== selection) {
			selection = current;
			if (current)
				untrack(() =>
					trackUsage(
						'case.select',
						app.studyView
							? { source: 'saved' }
							: app.activeMulti
								? { source: 'distribution' }
								: context(current as { id: string })
					)
				);
		}
	});
	$effect(() => {
		const mode = app.activeLocal?.displayMode === 'diagram' ? 'diagram' : app.displayMode;
		if (display !== mode) {
			display = mode;
			untrack(() => trackUsage('view.change', { view: mode }));
		}
	});
	$effect(() => {
		for (const c of [...app.cases, ...app.localCases]) {
			const pending = pendingSolves.get(c);
			const solution = c.solution;
			if (c.solving) {
				if (!pending)
					pendingSolves.set(c, {
						started: performance.now(),
						solution,
						errorRevision: app.errorRevision
					});
			} else if (pending) {
				pendingSolves.delete(c);
				seenSolutions.set(c, solution);
				untrack(() =>
					trackUsage('calculation.solve', {
						...context(c),
						calculation: c.formulation,
						backend: c.solveBackend === 'rust-server' ? 'server' : 'browser',
						result:
							solution && solution !== pending.solution
								? 'completed'
								: app.errorRevision !== pending.errorRevision
									? 'failed'
									: 'cancelled',
						duration_ms: performance.now() - pending.started,
						solve_ms: c.solveMs ?? undefined,
						iterations: c.iterations.length
					})
				);
			} else if (solution && seenSolutions.get(c) !== solution) {
				seenSolutions.set(c, solution);
				untrack(() =>
					trackUsage('calculation.solve', {
						...context(c),
						calculation: c.formulation,
						backend: 'cached',
						result: 'completed'
					})
				);
			}
			const previous = seenEdits.get(c);
			const next = { demand: c.deltas, capacity: c.ratings };
			seenEdits.set(c, next);
			if (previous)
				for (const edit of ['demand', 'capacity'] as const) {
					if (previous[edit] !== next[edit])
						untrack(() =>
							trackUsage('case.edit', {
								...context(c),
								edit,
								changed_count: Object.keys(next[edit]).length
							})
						);
				}
		}
	});
	$effect(() => {
		for (const c of app.multiCases) {
			const pending = pendingSolves.get(c);
			const result = c.result;
			if (c.solving && !pending)
				pendingSolves.set(c, {
					started: performance.now(),
					solution: result,
					errorRevision: app.errorRevision
				});
			else if (!c.solving && pending) {
				pendingSolves.delete(c);
				untrack(() =>
					trackUsage('calculation.solve', {
						source: 'distribution',
						calculation: 'mcpf',
						backend: 'browser',
						result:
							result && result !== pending.solution
								? 'completed'
								: app.errorRevision !== pending.errorRevision
									? 'failed'
									: 'cancelled',
						duration_ms: performance.now() - pending.started,
						solve_ms: c.solveMs ?? undefined,
						iterations: result?.iterations
					})
				);
			}
		}
	});

	$effect(() => {
		if (app.parsingFile && !importing)
			importing = {
				started: performance.now(),
				count: app.localCases.length + app.multiCases.length,
				errorRevision: app.errorRevision
			};
		else if (!app.parsingFile && importing) {
			const measured = importing;
			importing = null;
			untrack(() =>
				trackUsage('case.import', {
					result: measured.errorRevision === app.errorRevision ? 'completed' : 'failed',
					duration_ms: performance.now() - measured.started,
					item_count: Math.max(0, app.localCases.length + app.multiCases.length - measured.count)
				})
			);
		}
	});
	$effect(() => {
		if (app.sensitivityLoading && !sensitivity)
			sensitivity = { started: performance.now(), errorRevision: app.errorRevision };
		else if (!app.sensitivityLoading && sensitivity) {
			const measured = sensitivity;
			sensitivity = null;
			untrack(() =>
				trackUsage('calculation.sensitivity', {
					...(ctrl.activeSolvable ? context(ctrl.activeSolvable) : {}),
					calculation: ctrl.activeFormulation,
					result: measured.errorRevision === app.errorRevision ? 'completed' : 'failed',
					duration_ms: performance.now() - measured.started
				})
			);
		}
	});
</script>
