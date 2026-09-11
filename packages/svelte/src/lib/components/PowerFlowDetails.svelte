<script lang="ts">
	import { getController } from '../context.svelte.js';
	import { caseDeltas } from '../display.js';
	import { fmt } from '../format.js';

	const ctrl = getController();
	const active = $derived(ctrl.activeSolvable);
	const bus = $derived(ctrl.selectedBusData);
	const branch = $derived(ctrl.selectedBranchData);
	const voltage = $derived(active?.solution?.vm?.find((value) => value.bus === bus?.id)?.value);
	const angle = $derived(active?.solution?.va.find((value) => value.bus === bus?.id)?.value);
	const flow = $derived(active?.solution?.flows.find((value) => value.branch === branch?.id));
	const demand = $derived.by(() => {
		if (!bus || !active) return null;
		const deltas = caseDeltas(active);
		return bus.demand_mw + (deltas[bus.uid ?? bus.id] ?? deltas[bus.id] ?? 0);
	});
</script>

{#if bus || branch}
	<section class="equipment" aria-label="Power-flow equipment results">
		{#if bus}
			<h3>
				Bus {bus.id}{#if bus.name}<span>{bus.name}</span>{/if}
			</h3>
			<dl>
				{#if demand !== null}<div>
						<dt>Demand</dt>
						<dd>{fmt.format(demand)} MW</dd>
					</div>{/if}
				{#if voltage != null}<div>
						<dt>Voltage</dt>
						<dd>{voltage.toFixed(4)} pu</dd>
					</div>{/if}
				{#if angle != null}<div>
						<dt>Voltage angle</dt>
						<dd>{angle.toFixed(4)} rad</dd>
					</div>{/if}
			</dl>
		{:else if branch}
			<h3>Line {branch.id}<span>Bus {branch.from} to bus {branch.to}</span></h3>
			<dl>
				<div>
					<dt>Capacity</dt>
					<dd>{branch.rate_mw > 0 ? `${fmt.format(branch.rate_mw)} MVA` : 'Unlimited'}</dd>
				</div>
				{#if flow}
					<div>
						<dt>Active power</dt>
						<dd>{fmt.format(flow.mw)} MW</dd>
					</div>
					{#if branch.rate_mw > 0}<div>
							<dt>Loading</dt>
							<dd>{fmt.format(flow.loading * 100)}%</dd>
						</div>{/if}
				{/if}
			</dl>
		{/if}
	</section>
{/if}

<style>
	.equipment {
		border-top: 1px solid var(--line);
		margin-top: 14px;
		padding-top: 14px;
	}
	h3 {
		margin: 0 0 12px;
		font-size: 13px;
		font-weight: 600;
	}
	h3 span {
		display: block;
		margin-top: 3px;
		font-size: 11px;
		font-weight: 400;
		color: var(--text-secondary);
	}
	dl {
		margin: 0;
		display: grid;
		gap: 6px;
		font: 12px/1.5 var(--font-mono);
	}
	dl > div {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		gap: 12px;
	}
	dt {
		color: var(--text-secondary);
	}
	dd {
		margin: 0;
		text-align: right;
	}
</style>
