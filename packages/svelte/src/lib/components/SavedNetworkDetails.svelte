<script lang="ts">
	import { getAppState } from '../context.svelte.js';
	import { flowSummary, resultStats } from '../diagram/verdict.js';
	import {
		atLimit,
		degrees,
		fmt,
		formulationLabel,
		quantity,
		signed,
		splitName
	} from '../format.js';
	import BusPicker from './BusPicker.svelte';
	import ModelDetails from './ModelDetails.svelte';
	import SolveCard from './SolveCard.svelte';
	import StatBoxes from './StatBoxes.svelte';

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

	/** The case's display name, as in the header: the hosted or local case it was saved from,
	 * else the network's own name without a file prefix. */
	const name = $derived.by((): [string, string] => {
		if (!saved) return ['', ''];
		const hosted = app.cases.find((c) => c.id === saved.caseId);
		if (hosted) return splitName(hosted.name);
		const local = app.localCases.find((c) => c.id === saved.caseId);
		if (local) return [local.label, ''];
		return splitName(saved.network.name.replace(/^case_?/i, ''));
	});

	/** The same boxes as the live result: an OPF's operational cost and lines at limit, a power
	 * flow's losses, heaviest line and voltage range. */
	const stats = $derived.by(() => {
		const solution = saved?.solution;
		if (!saved || !solution) return [];
		const constraints = solution.branch_constraints;
		const flows = solution.flows ?? [];
		const binding = constraints
			? constraints.filter((row) => row.binding).length
			: flows.filter(atLimit).length;
		return resultStats({
			formulation: saved.formulation,
			objective: solution.objective ?? null,
			binding,
			...flowSummary(
				solution.vm,
				flows.map((f) => ({ loading: f.loading, from: f.pf, to: f.pt }))
			)
		});
	});

	/** Every bus colouring a saved state can have; the ones its result lacks are disabled. */
	const modes = $derived([
		{ mode: 'price' as const, label: 'Price', available: !!saved?.solution?.lmp?.length },
		{ mode: 'angle' as const, label: 'Angle', available: !!saved?.solution?.va?.length },
		{
			mode: 'voltage' as const,
			label: 'Voltage',
			available: !!(saved?.solution?.vm?.length || saved?.solution?.w?.length)
		}
	]);

	function live() {
		app.studyView = null;
		app.selectedBus = null;
		app.selectedBranch = null;
	}
</script>

{#if saved}
	<section aria-label="Saved network details">
		<h2>
			{name[0]}{#if name[1]}<span class="region">{name[1]}</span>{/if}
		</h2>
		<StatBoxes {stats} />
		<dl>
			<div>
				<dt>Saved state</dt>
				<dd>{saved.label}</dd>
			</div>
			<div>
				<dt>Calculation</dt>
				<dd>{formulationLabel(saved.formulation)}</dd>
			</div>
			<div>
				<dt>Buses</dt>
				<dd>{saved.network.buses.length.toLocaleString()}</dd>
			</div>
			<div>
				<dt>Lines</dt>
				<dd>{saved.network.branches.length.toLocaleString()}</dd>
			</div>
		</dl>
		<SolveCard embedded />
		<button class="reset live" onclick={live}>Return to live case</button>
		<ModelDetails details={saved.network.model_details} />
		<BusPicker inline />
		{#if saved.solution && modes.some((m) => m.available)}
			<div class="mode display-mode values">
				<div class="segment" aria-label="Bus color variable">
					{#each modes as option (option.mode)}
						<button
							type="button"
							class={{ active: app.displayMode === option.mode }}
							aria-pressed={app.displayMode === option.mode}
							disabled={!option.available}
							onclick={() => (app.displayMode = option.mode)}>{option.label}</button
						>
					{/each}
				</div>
			</div>
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
							<dt>Price</dt>
							<dd>{fmt.format(lmp)}<small>$/MWh</small></dd>
						</div>{/if}
					{#if voltage != null || squaredVoltage != null}<div>
							<dt>Voltage</dt>
							<dd>{quantity(voltage ?? Math.sqrt(Math.max(0, squaredVoltage!)), 'pu')}</dd>
						</div>{/if}
					{#if angle != null}<div>
							<dt>Voltage angle</dt>
							<dd>{degrees(angle)}</dd>
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
		margin: 0 0 var(--space-lg);
		font-size: var(--fs-heading);
		font-weight: 600;
	}
	dl {
		margin: 0;
		font-size: var(--fs-data);
		font-variant-numeric: tabular-nums;
	}
	dl > div {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		gap: var(--space-lg);
		padding: var(--space-3xs) 0;
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
		font-size: 12px;
		color: var(--text-secondary);
	}
	.live {
		margin: 0 0 16px;
	}
	.values {
		margin: 16px 0;
	}
	.segment button:disabled {
		color: var(--text-tertiary);
		background: none;
		cursor: not-allowed;
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
		font-size: 13px;
		font-weight: 400;
	}
</style>
