<script lang="ts">
	import { getController } from '../context.svelte.js';
	import { caseDeltas } from '../display.js';
	import { fmt, quantity } from '../format.js';

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
						<dt>Demand (MW)</dt>
						<dd>{fmt.format(demand)}</dd>
					</div>{/if}
				{#if voltage != null}<div>
						<dt>Voltage (pu)</dt>
						<dd>{quantity(voltage, '', { decimals: 4 })}</dd>
					</div>{/if}
				{#if angle != null}<div>
						<dt>Voltage angle (°)</dt>
						<dd>{quantity((angle * 180) / Math.PI, '', { decimals: 2 })}</dd>
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
						<dt>Active power (MW)</dt>
						<dd>{fmt.format(flow.mw)}</dd>
					</div>
					{#if branch.rate_mw > 0}<div>
							<dt>Loading (%)</dt>
							<dd>{fmt.format(flow.loading * 100)}</dd>
						</div>{/if}
				{/if}
			</dl>
		{/if}
	</section>
{/if}

<style>
	.equipment {
		border-top: 1px solid var(--line);
		margin-top: var(--space-xl);
		padding-top: var(--space-xl);
	}
	h3 {
		margin: 0 0 var(--space-lg);
		font-size: var(--fs-heading);
		font-weight: 600;
	}
	h3 span {
		display: block;
		margin-top: var(--space-3xs);
		font-size: var(--fs-label);
		font-weight: 400;
		color: var(--text-secondary);
	}
	dl {
		margin: 0;
		display: grid;
		gap: var(--space-xs);
		font-size: var(--fs-data);
		line-height: var(--lh-normal);
	}
	dl > div {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		gap: var(--space-lg);
	}
	dt {
		color: var(--text-secondary);
	}
	dd {
		margin: 0;
		text-align: right;
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums;
	}
</style>
