<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { lineName } from '../format.js';
	import { gaugeText } from '../diagram/transmission-results.js';

	const app = getAppState();
	const ctrl = getController();

	// The active result's binding lines (see `ctrl.bindingRule`), joined to the network for
	// identity, heaviest first. Each row is a button: the keyboard path for branch selection.
	const bindingLines = $derived.by(() => {
		const c = ctrl.activeSolvable;
		const rule = ctrl.bindingRule;
		if (!c?.network || !c.solution || !rule) return [];
		const byId = new Map(c.network.branches.map((b) => [b.id, b]));
		return c.solution.flows
			.filter(rule.binds)
			.flatMap((f) => {
				const branch = byId.get(f.branch);
				return branch ? [{ branch, loading: f.loading }] : [];
			})
			.sort((a, b) => b.loading - a.loading);
	});

	/** Rows shown before "Show all": the panel scrolls as one, with no list scrolling inside it. */
	const FIRST = 5;
	/** The case whose list is unfolded: another case starts folded again. */
	let expandedFor = $state<string | null>(null);
	const expanded = $derived(!!ctrl.activeSolvable && expandedFor === ctrl.activeSolvable.id);
	const shown = $derived.by(() => {
		if (expanded || bindingLines.length <= FIRST) return bindingLines;
		const first = bindingLines.slice(0, FIRST);
		// The selected line stays in view, even when it is further down the list.
		const selected = bindingLines.find(
			(row, i) => i >= FIRST && row.branch.id === app.selectedBranch
		);
		return selected ? [...first, selected] : first;
	});

	function select(branchId: number) {
		const c = ctrl.activeSolvable;
		if (!c) return;
		if (ctrl.isBackendCase(c)) ctrl.selectBranch(c.id, branchId, { focus: true });
		else ctrl.selectLocalBranch(c.id, branchId, { focus: true });
	}
</script>

{#if bindingLines.length > 0}
	<section class="binding-lines" aria-labelledby="binding-lines-title">
		<h3 id="binding-lines-title">
			<span>{ctrl.bindingRule?.label}</span>
			<span class="count">{bindingLines.length.toLocaleString('en-US')}</span>
		</h3>
		<ul>
			{#each shown as { branch, loading } (branch.id)}
				<li>
					<button
						class:selected={branch.id === app.selectedBranch}
						aria-current={branch.id === app.selectedBranch ? 'true' : undefined}
						onclick={() => select(branch.id)}
					>
						<span>{lineName(branch)}</span>
						<span class="load">{gaugeText(loading)}</span>
					</button>
				</li>
			{/each}
		</ul>
		{#if bindingLines.length > FIRST}
			<button
				class="more"
				aria-expanded={expanded}
				onclick={() => (expandedFor = expanded ? null : (ctrl.activeSolvable?.id ?? null))}
			>
				{expanded ? 'Show fewer' : `Show all ${bindingLines.length.toLocaleString('en-US')}`}
			</button>
		{/if}
	</section>
{/if}

<style>
	/* Under the stat boxes, at the panel's own edge, like the rows below it. */
	.binding-lines {
		margin: 0 0 var(--space-lg);
	}

	h3 {
		display: flex;
		justify-content: space-between;
		gap: var(--space-lg);
		margin: 0 0 var(--space-2xs);
		font-size: var(--fs-data);
		font-weight: 600;
		color: var(--ink);
	}

	.count {
		font-weight: 400;
		color: var(--text-secondary);
		font-variant-numeric: tabular-nums;
	}

	/* Rows reach past the text by a few pixels, so a hover or selection has room. */
	ul {
		margin: 0 calc(-1 * var(--space-xs));
		padding: 0;
		list-style: none;
	}

	li button {
		display: flex;
		align-items: baseline;
		gap: var(--space-lg);
		width: 100%;
		min-height: 24px;
		padding: var(--space-3xs) var(--space-xs);
		text-align: left;
		font-size: var(--fs-data);
		color: var(--ink);
		background: none;
		border: 0;
		border-radius: 0;
		cursor: pointer;
	}

	/* The rule above each row spans the text column, as the panel's other rules do; the
	   button's hover area reaches past it. */
	li {
		position: relative;
	}

	li::before {
		content: '';
		position: absolute;
		top: 0;
		left: var(--space-xs);
		right: var(--space-xs);
		border-top: 1px solid var(--line);
		pointer-events: none;
	}

	.load {
		margin-left: auto;
		text-align: right;
		color: var(--text-danger);
		font-variant-numeric: tabular-nums;
	}

	li button:hover {
		background: var(--surface-hover);
		text-decoration: none;
		box-shadow: none;
	}

	li button.selected {
		background: var(--surface-hover);
		box-shadow: inset 2px 0 0 var(--accent);
	}

	.more {
		margin-top: var(--space-2xs);
		padding: 0;
		font-size: var(--fs-label);
		color: var(--accent);
		background: none;
		border: 0;
		cursor: pointer;
	}

	.more:hover {
		text-decoration: underline;
	}

	/* Each row is a tap target that reframes the map, so give it a finger's worth
	   of height. */
	@media (hover: none), (pointer: coarse) {
		li button {
			min-height: 44px;
			align-items: center;
			padding-block: var(--space-xs);
		}

		.more {
			min-height: 44px;
		}
	}
</style>
