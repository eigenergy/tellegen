<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';

	const app = getAppState();
	const ctrl = getController();
	const FOCUS_FIRST_SENSITIVITY_DELAY_MS = 700;

	// The active case's binding branches (loading at the thermal limit), joined to
	// the network for identity. Renders nothing when none bind. Each row is a
	// button: the keyboard path for branch selection, mirroring what clicking the
	// line on the map does.
	const bindingLines = $derived.by(() => {
		const c = ctrl.activeSolvable;
		if (!c?.network || !c.solution) return [];
		const byId = new Map(c.network.branches.map((b) => [b.id, b]));
		return c.solution.flows
			.filter((f) => f.loading >= 0.999)
			.flatMap((f) => {
				const branch = byId.get(f.branch);
				return branch ? [{ branch, loading: f.loading }] : [];
			});
	});

	function select(branchId: number) {
		const c = ctrl.activeSolvable;
		if (!c) return;
		if (ctrl.isBackendCase(c)) ctrl.selectBranch(c.id, branchId, FOCUS_FIRST_SENSITIVITY_DELAY_MS);
		else ctrl.selectLocalBranch(c.id, branchId, FOCUS_FIRST_SENSITIVITY_DELAY_MS);
		app.requestBranchFocus(c.id, branchId);
	}
</script>

{#if bindingLines.length > 0}
	<div class="binding-lines">
		<p
			class="mono dim head"
			title="lines loaded to their thermal rating; select one to see &part;LMP/&part;rating"
		>
			binding lines
		</p>
		<ul class="mono">
			{#each bindingLines as { branch, loading } (branch.id)}
				<li>
					<button
						class="mono"
						class:selected={app.selectedBranch === branch.id}
						aria-pressed={app.selectedBranch === branch.id}
						onclick={() => select(branch.id)}
					>
						line {branch.from}&#8201;&ndash;&#8201;{branch.to} &middot; {Math.round(
							loading * 100
						)}%
					</button>
				</li>
			{/each}
		</ul>
	</div>
{/if}

<style>
	.binding-lines {
		margin-top: 10px;
	}

	.head {
		font-size: 10.5px;
		margin: 0 0 4px;
	}

	ul {
		margin: 0;
		padding: 0;
		list-style: none;
		max-height: 118px;
		overflow-y: auto;
	}

	li button {
		width: 100%;
		padding: 3px 6px 3px 8px;
		text-align: left;
		font-size: 12px;
		color: var(--ink);
		background: none;
		border: 0;
		border-top: 1px solid var(--line);
		border-radius: 0;
		cursor: pointer;
	}

	li button:hover {
		background: var(--accent-soft);
	}

	li button.selected {
		color: var(--text-accent);
		background:
			linear-gradient(90deg, rgba(177, 104, 27, 0.16), rgba(177, 104, 27, 0.05)),
			var(--accent-soft);
		box-shadow: inset 3px 0 0 var(--accent);
		font-weight: 600;
	}
</style>
