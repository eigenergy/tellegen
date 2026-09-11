<script lang="ts">
	import { getAppState } from '../context.svelte.js';
	import { fmt, formulationLabel, signed } from '../format.js';
	import BusPicker from './BusPicker.svelte';
	import ModelDetails from './ModelDetails.svelte';

	const app = getAppState();
	const saved = $derived(app.studyView);
	const bus = $derived(saved?.network.buses.find((value) => value.id === app.selectedBus));
	const branch = $derived(saved?.network.branches.find((value) => value.id === app.selectedBranch));
	const lmp = $derived(saved?.solution?.lmp?.find((value) => value.bus === bus?.id)?.value);
	const voltage = $derived(saved?.solution?.vm?.find((value) => value.bus === bus?.id)?.value);
	const squaredVoltage = $derived(
		saved?.solution?.w?.find((value) => value.bus === bus?.id)?.value
	);
	const angle = $derived(saved?.solution?.va?.find((value) => value.bus === bus?.id)?.value);
	const flow = $derived(saved?.solution?.flows?.find((value) => value.branch === branch?.id));
	const baseDemand = $derived(bus ? saved?.baseDemandMw?.[String(bus.id)] : undefined);
	function live() {
		app.studyView = null;
		app.selectedBus = null;
		app.selectedBranch = null;
	}
</script>

{#if saved}
	<section aria-label="Saved network details">
		<h2>{saved.network.name}</h2>
		<p class="state-label">{saved.label}<span>Saved state</span></p>
		<dl>
			<div>
				<dt>Buses</dt>
				<dd>{saved.network.buses.length.toLocaleString()}</dd>
			</div>
			<div>
				<dt>Lines</dt>
				<dd>{saved.network.branches.length.toLocaleString()}</dd>
			</div>
			<div>
				<dt>Calculation</dt>
				<dd>{formulationLabel(saved.formulation)}</dd>
			</div>
			<div>
				<dt>Result</dt>
				<dd>{saved.solution?.status ?? 'Not solved'}</dd>
			</div>
			{#if saved.solution?.objective != null}
				<div>
					<dt>OPF objective</dt>
					<dd>{fmt.format(saved.solution.objective)}</dd>
				</div>
			{/if}
		</dl>
		<button class="live" onclick={live}>Return to live case</button>
		<ModelDetails details={saved.network.model_details} />
		<BusPicker inline />
		{#if saved.solution}
			<label class="map-values"
				>{saved.network.coordinate_space === 'diagram' ? 'Diagram values' : 'Map values'}
				<select bind:value={app.displayMode}>
					{#if saved.solution.lmp?.length}<option value="price">LMP</option>{/if}
					{#if saved.solution.va?.length}<option value="angle">Voltage angle</option>{/if}
					{#if saved.solution.vm?.length || saved.solution.w?.length}<option value="voltage"
							>Voltage magnitude</option
						>{/if}
				</select>
			</label>
		{/if}
		{#if bus}
			<div class="equipment">
				<h3>
					Bus {bus.id}{#if bus.name}<span>{bus.name}</span>{/if}
				</h3>
				<dl>
					{#if baseDemand != null}<div>
							<dt>Base demand</dt>
							<dd>{fmt.format(baseDemand)} MW</dd>
						</div>{/if}
					<div>
						<dt>Current demand</dt>
						<dd>{fmt.format(bus.demand_mw)} MW</dd>
					</div>
					{#if baseDemand != null}<div>
							<dt>Change</dt>
							<dd>{signed(bus.demand_mw - baseDemand)} MW</dd>
						</div>{/if}
					{#if lmp != null}<div>
							<dt>LMP</dt>
							<dd>{fmt.format(lmp)}<small>objective units/MW</small></dd>
						</div>{/if}
					{#if voltage != null || squaredVoltage != null}<div>
							<dt>Voltage</dt>
							<dd>{(voltage ?? Math.sqrt(Math.max(0, squaredVoltage!))).toFixed(4)} pu</dd>
						</div>{/if}
					{#if angle != null}<div>
							<dt>Voltage angle</dt>
							<dd>{angle.toFixed(4)} rad</dd>
						</div>{/if}
				</dl>
			</div>
		{:else if branch}
			<div class="equipment">
				<h3>Line {branch.id}<span>Bus {branch.from} to bus {branch.to}</span></h3>
				<dl>
					<div>
						<dt>Capacity</dt>
						<dd>
							{branch.rate_mw > 0
								? `${fmt.format(branch.rate_mw)} ${saved.formulation === 'dcopf' ? 'MW' : 'MVA'}`
								: 'Unlimited'}
						</dd>
					</div>
					{#if flow}<div>
							<dt>Active power</dt>
							<dd>{fmt.format(flow.pf)} MW</dd>
						</div>
						<div>
							<dt>Loading</dt>
							<dd>{fmt.format(flow.loading * 100)}%</dd>
						</div>{/if}
				</dl>
			</div>
		{/if}
	</section>
{/if}

<style>
	h2 {
		margin: 0 0 8px;
		font-size: 15px;
		font-weight: 600;
	}
	.state-label {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		margin: 0 0 16px;
		font-size: 12px;
	}
	.state-label span {
		flex: none;
		color: var(--text-secondary);
		font-size: 11px;
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
	small {
		display: block;
		font-size: 10px;
		color: var(--text-secondary);
	}
	.live {
		margin: 16px 0;
	}
	button,
	select {
		border: 1px solid var(--line);
		border-radius: 4px;
		color: var(--ink);
		background: var(--paper);
		padding: 6px 8px;
		font: 12px/1.4 var(--font-display);
	}
	button {
		cursor: pointer;
	}
	button:focus-visible,
	select:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
	}
	.map-values {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 12px;
		margin: 16px 0;
		font-size: 12px;
	}
	.equipment {
		border-top: 1px solid var(--line);
		margin-top: 16px;
		padding-top: 16px;
	}
	h3 {
		margin: 0 0 12px;
		font-size: 13px;
		font-weight: 600;
	}
	h3 span {
		display: block;
		margin-top: 3px;
		color: var(--text-secondary);
		font-size: 11px;
		font-weight: 400;
	}
</style>
