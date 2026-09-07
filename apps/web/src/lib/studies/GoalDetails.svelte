<script lang="ts">
	import type { GoalRevision, Network, StudyObjective } from '@tellegen/engine';
	import { busLabel, branchLabel, elementKey, objectiveLabel } from './goal-form.js';
	let {
		goal,
		network,
		ratingUnit
	}: { goal: GoalRevision; network?: Network | null; ratingUnit: string } = $props();
	let open = $state(false);
	let page = $state(0);
	const pageSize = 20;
	const busNames = $derived(
		new Map(network?.buses.map((bus) => [String(elementKey(bus)), busLabel(bus)]) ?? [])
	);
	const branchNames = $derived(
		new Map(
			network?.branches.map((branch) => [String(elementKey(branch)), branchLabel(branch)]) ?? []
		)
	);
	const terms = $derived.by(() => {
		const rows: Array<{ id: string; quantity: string; element: string; weight: number }> = [];
		function visit(objective: StudyObjective, path: string, context: string) {
			switch (objective.kind) {
				case 'weighted_observable':
					for (const [index, weight] of objective.weights.entries()) {
						const key = String(weight.element);
						const quantity =
							'Price' in objective.operand
								? 'LMP'
								: 'Voltage' in objective.operand
									? 'Voltage'
									: 'Flow' in objective.operand
										? 'Line flow'
										: 'Generation';
						rows.push({
							id: `${path}:${index}`,
							quantity: `${context}${quantity}`,
							element:
								'Flow' in objective.operand
									? (branchNames.get(key) ?? key)
									: (busNames.get(key) ?? key),
							weight: weight.weight
						});
					}
					break;
				case 'sum':
					objective.terms.forEach((term, index) => visit(term, `${path}/${index}`, context));
					break;
				case 'scale':
					visit(
						objective.expression,
						`${path}/scale`,
						`${context}${objective.factor.toPrecision(3)} times `
					);
					break;
				case 'squared_target':
					visit(
						objective.expression,
						`${path}/target`,
						`${context}squared deviation from ${objective.target}, `
					);
					break;
				case 'intervention_penalty':
					rows.push({
						id: path,
						quantity: `Penalty: ${objective.linear}x + ${objective.quadratic}x²`,
						element: objective.decision,
						weight: 1
					});
					break;
			}
		}
		if (open) visit(goal.objective, 'objective', '');
		return rows;
	});
	const pageCount = $derived(
		Math.max(1, Math.ceil(Math.max(terms.length, goal.decisions.variables.length) / pageSize))
	);
	const currentPage = $derived(Math.min(page, pageCount - 1));
	const number = (value: number) =>
		value.toLocaleString(undefined, { maximumSignificantDigits: 5 });
</script>

<details bind:open>
	<summary>Equipment and objective details</summary>
	{#if open}
		<p>{objectiveLabel(goal.objective)}</p>
		<table>
			<thead><tr><th>Quantity</th><th>Element</th><th>Weight</th></tr></thead><tbody>
				{#each terms.slice(currentPage * pageSize, (currentPage + 1) * pageSize) as term (term.id)}<tr
						><td>{term.quantity}</td><td>{term.element}</td><td>{number(term.weight)}</td></tr
					>{/each}
			</tbody>
		</table>
		<p>Permitted changes</p>
		<table>
			<thead><tr><th>Element</th><th>Min</th><th>Max</th><th>Step</th><th>Unit</th></tr></thead
			><tbody>
				{#each goal.decisions.variables.slice(currentPage * pageSize, (currentPage + 1) * pageSize) as variable (variable.id)}<tr
						><td
							>{(variable.intervention === 'branch_rating' ? branchNames : busNames).get(
								String(variable.element)
							) ?? variable.element}</td
						><td>{number(variable.lower)}</td><td>{number(variable.upper)}</td><td
							>{number(variable.increment)}</td
						><td>{variable.intervention === 'branch_rating' ? ratingUnit : 'MW'}</td></tr
					>{/each}
			</tbody>
		</table>
		{#if pageCount > 1}<div class="pagination">
				<button disabled={currentPage === 0} onclick={() => (page = currentPage - 1)}
					>Previous</button
				><span>Page {currentPage + 1} of {pageCount}</span><button
					disabled={currentPage + 1 >= pageCount}
					onclick={() => (page = currentPage + 1)}>Next</button
				>
			</div>{/if}
	{/if}
</details>

<style>
	details {
		border-top: 1px solid var(--line);
		padding: 12px 0;
	}
	summary {
		cursor: pointer;
		font-size: 12px;
	}
	p {
		margin: 16px 0 10px;
		font-size: 12px;
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 10px;
	}
	th,
	td {
		padding: 7px 4px;
		border-bottom: 1px solid var(--line);
		text-align: right;
		font-variant-numeric: tabular-nums;
		overflow-wrap: anywhere;
	}
	th:first-child,
	td:first-child {
		text-align: left;
	}
	th {
		font-weight: 500;
	}
	.pagination {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		margin: 12px 0;
		font-size: 11px;
	}
	button {
		padding: 4px 6px;
		font: inherit;
		border: 1px solid var(--line);
		background: transparent;
		border-radius: 4px;
		cursor: pointer;
		color: inherit;
	}
	button:disabled {
		opacity: 0.45;
		cursor: default;
	}
	button:focus-visible,
	summary:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
	}
</style>
