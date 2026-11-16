<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { lineName } from '../format.js';
	import { gaugeText } from '../diagram/transmission-results.js';

	const app = getAppState();
	const ctrl = getController();

	// The active result's binding lines (see `ctrl.bindingRule`), joined to the network for
	// identity. Each row is a button: the keyboard path for branch selection. The selected line
	// leaves the list: the inspector below names it.
	const bindingLines = $derived.by(() => {
		const c = ctrl.activeSolvable;
		const rule = ctrl.bindingRule;
		if (!c?.network || !c.solution || !rule) return [];
		const byId = new Map(c.network.branches.map((b) => [b.id, b]));
		return c.solution.flows.filter(rule.binds).flatMap((f) => {
			const branch = byId.get(f.branch);
			return branch && branch.id !== app.selectedBranch ? [{ branch, loading: f.loading }] : [];
		});
	});

	function select(branchId: number) {
		const c = ctrl.activeSolvable;
		if (!c) return;
		if (ctrl.isBackendCase(c)) ctrl.selectBranch(c.id, branchId, { focus: true });
		else ctrl.selectLocalBranch(c.id, branchId, { focus: true });
	}
</script>

{#if bindingLines.length > 0}
	<!-- The lines the box above counts, by name: the box is their heading. -->
	<div class="binding-lines">
		<ul aria-label={ctrl.bindingRule?.label}>
			{#each bindingLines as { branch, loading } (branch.id)}
				<li>
					<button onclick={() => select(branch.id)}>
						<span>{lineName(branch)}</span>
						<span class="load">{gaugeText(loading)}</span>
					</button>
				</li>
			{/each}
		</ul>
	</div>
{/if}

<style>
	/* Under the stat boxes, at the panel's own edge, like the rows below it. */
	.binding-lines {
		margin: 0 0 var(--space-lg);
		border-bottom: 1px solid var(--line);
	}

	/* Rows reach past the text by a few pixels, so a hover or selection has room. */
	ul {
		margin: 0 calc(-1 * var(--space-xs));
		padding: 0;
		list-style: none;
		max-height: 118px;
		overflow-y: auto;
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
	/* Each row is a tap target that reframes the map, so give it a finger's worth
	   of height and show a couple more rows before the list scrolls. */
	@media (hover: none), (pointer: coarse) {
		ul {
			max-height: 176px;
		}

		li button {
			min-height: 44px;
			align-items: center;
			padding-block: var(--space-xs);
		}
	}
</style>
