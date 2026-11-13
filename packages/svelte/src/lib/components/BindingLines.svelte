<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';

	const app = getAppState();
	const ctrl = getController();

	// The active result's binding lines (see `ctrl.bindingRule`), joined to the network for
	// identity. Each row is a button: the keyboard path for branch selection.
	const bindingLines = $derived.by(() => {
		const c = ctrl.activeSolvable;
		const rule = ctrl.bindingRule;
		if (!c?.network || !c.solution || !rule) return [];
		const byId = new Map(c.network.branches.map((b) => [b.id, b]));
		return c.solution.flows.filter(rule.binds).flatMap((f) => {
			const branch = byId.get(f.branch);
			return branch ? [{ branch, loading: f.loading }] : [];
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
	<div class="binding-lines">
		<p class="dim head">{ctrl.bindingRule?.label}</p>
		<ul>
			{#each bindingLines as { branch, loading } (branch.id)}
				<li>
					<button
						class={{ selected: app.selectedBranch === branch.id }}
						aria-pressed={app.selectedBranch === branch.id}
						onclick={() => select(branch.id)}
					>
						Line <span class="mono">{branch.from}&#8201;&ndash;&#8201;{branch.to}</span>
						<span class="mono load">{Math.round(loading * 100)}%</span>
					</button>
				</li>
			{/each}
		</ul>
	</div>
{/if}

<style>
	.binding-lines {
		margin-top: var(--space-md);
	}

	.head {
		font-size: var(--fs-data);
		margin: 0 0 var(--space-2xs);
	}

	ul {
		margin: 0;
		padding: 0;
		list-style: none;
		max-height: 118px;
		overflow-y: auto;
	}

	li button {
		display: flex;
		gap: var(--space-2xs);
		width: 100%;
		padding: var(--space-3xs) var(--space-xs) var(--space-3xs) var(--space-sm);
		text-align: left;
		font-size: var(--fs-label);
		color: var(--ink);
		background: none;
		border: 0;
		border-top: 1px solid var(--line);
		border-radius: 0;
		cursor: pointer;
	}

	.load {
		margin-left: auto;
		text-align: right;
	}

	li button:hover {
		background: var(--accent-soft);
	}

	li button.selected {
		color: var(--text-accent);
		background:
			linear-gradient(90deg, rgb(var(--accent-rgb) / 0.16), rgb(var(--accent-rgb) / 0.05)),
			var(--accent-soft);
		box-shadow: inset 3px 0 0 var(--accent);
		font-weight: 600;
	}

	/* Each row is a tap target that reframes the map, so give it a finger's worth
	   of height and show a couple more rows before the list scrolls. */
	@media (hover: none), (pointer: coarse) {
		ul {
			max-height: 176px;
		}

		li button {
			min-height: 44px;
			padding-block: var(--space-xs);
		}
	}
</style>
