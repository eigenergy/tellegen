<script lang="ts">
	import type { operatingExceptions } from '../../diagram/transmission-inspection.js';
	import { formulationLabel } from '../../format.js';
	import {
		voltageGradient,
		type operatingContext,
		type stationVoltageSummary
	} from '../../diagram/transmission-results.js';
	import { TERMS, operatingLead } from '../../diagram/transmission-explain.js';
	import TxPanelHead from './TxPanelHead.svelte';
	let {
		formulation,
		context,
		voltageAvailable,
		counts,
		warning,
		onwarning,
		station,
		shading = false,
		brief = false,
		onselectbus
	}: {
		formulation: Parameters<typeof formulationLabel>[0];
		context: ReturnType<typeof operatingContext>;
		voltageAvailable: boolean;
		counts: ReturnType<typeof operatingExceptions>['counts'];
		warning: number;
		onwarning: (value: number) => void;
		station: ({ name: string } & ReturnType<typeof stationVoltageSummary>) | null;
		onselectbus: (id: number) => void;
		/** Whether the area shading shows voltage against the bus limits. */
		shading?: boolean;
		/** Collapsed to the heading and the counts that matter. */
		brief?: boolean;
	} = $props();
	const lead = $derived(
		operatingLead({
			hasResult: context.solved,
			voltageSolved: context.voltage,
			counts,
			warning
		})
	);
</script>

<section class="operating-panel" aria-label="Operating limits">
	<TxPanelHead title="Operating limits" {formulation} {context} {lead} view="operating" {brief} />
	{#if !brief}{#if context.voltage && voltageAvailable}<div class="voltage-key">
				<span>Voltage</span><span>0.90</span><i style:background={voltageGradient}></i><span
					>1.10 pu</span
				>
			</div>{/if}
		{#if shading}<p class="freshness">
				Interpolated shading: near the <span class="low">minimum</span> or
				<span class="high">maximum</span> voltage limit
			</p>{/if}{/if}
	<div class="counts" aria-label="Operating limit counts">
		{#if voltageAvailable}<span class={[counts.low + counts.high > 0 && 'violation']}
				>↓ {counts.low} low, ↑ {counts.high} high</span
			>
		{:else}<span
				title={context.voltage
					? undefined
					: 'DC power flow treats every voltage as 1 pu, so voltage magnitudes are not solved.'}
				>{context.voltage ? 'Voltage results unavailable' : 'Voltage not solved (DC)'}</span
			>{/if}
		<span
			class={[counts.overload > 0 && 'violation']}
			title="Lines carrying more than their rating."
			>{counts.overload} overload{counts.overload === 1 ? '' : 's'}</span
		>
		<span class={[counts.warning + counts.atLimit > 0 && 'warning']}
			><span title="{TERMS.warning} The threshold is {warning}%.">{counts.warning} warning</span>,
			<span title={TERMS.atRating}>{counts.atLimit} at rating</span></span
		>
		<span hidden={brief}
			>{context.voltage ? `${counts.voltageUnknown} voltage unknown, ` : ''}<span
				title={TERMS.loadingUnknown}
				>{counts.flowMissing} missing flows, {counts.ratingUnknown} unknown ratings</span
			>,
			<span title={TERMS.outOfService}>{counts.inactive} out of service</span>{counts.disconnected
				? `, ${counts.disconnected} de-energized`
				: ''}</span
		>
	</div>
	<details class="settings" hidden={brief}>
		<summary>Key and settings</summary>
		<div class="settings-body">
			{#if context.voltage}
				<p>
					↓ marks a bus below and ↑ a bus above its case voltage limits. Faded buses lack a voltage
					or limits; × marks a de-energized bus.
				</p>{/if}
			<p>
				<span title={TERMS.rating}>Loading</span> is {context.basisText} (<code
					>{context.basis}</code
				>). Gauges show 0–125%, with a tick at 100%, and a leader to their branch. Values above the
				gauge range keep their numeric label. A heavy amber outline means at rating; a filled red
				gauge means overloaded.
			</p>
			<label
				>Loading warning <input
					aria-label="Loading warning percent"
					type="number"
					min="0"
					max="99"
					step="1"
					value={warning}
					onchange={(e) => {
						const value = e.currentTarget.valueAsNumber;
						if (e.currentTarget.validity.valid && Number.isFinite(value)) onwarning(value);
						else e.currentTarget.value = String(warning);
					}}
				/> %</label
			>
			<p>
				Per unit (pu) compares voltage with its nominal kV: 1.00 pu is nominal, 0.95 pu is 5% below
				it.
			</p>
			<p>
				Lines are grey below {warning}% of their rating, amber from {warning}% to 100%, and red
				above 100%. At rating does not mean economically binding.
			</p>
			<p>
				Unknown or synthesized ratings are faded; out-of-service lines are dashed. Nominal kV is
				shown in labels and equipment details; voltage-level colours are available in the view
				selector.
			</p>
		</div>
	</details>
	{#if station && context.voltage && !brief}
		<details class="station">
			<summary
				><strong>Station {station.name}</strong>
				{#if station.min && station.max}{station.min.value.toFixed(4)}–{station.max.value.toFixed(
						4
					)} pu{:else}voltage unavailable{/if}
				{#if station.worst}<span class="violation">
						{station.worst.assessment.state === 'low' ? '↓' : '↑'} bus {station.worst.bus}</span
					>{/if}</summary
			>
			<div class="station-values" aria-label="Station voltage range">
				{#if station.min}<button onclick={() => onselectbus(station.min!.bus)}
						>Minimum: bus {station.min.bus}, {station.min.value.toFixed(4)} pu</button
					>{/if}
				{#if station.max}<button onclick={() => onselectbus(station.max!.bus)}
						>Maximum: bus {station.max.bus}, {station.max.value.toFixed(4)} pu</button
					>{/if}
				{#if station.worst}<button class="violation" onclick={() => onselectbus(station.worst!.bus)}
						>Worst violation: bus {station.worst.bus}, {station.worst.assessment.exceedance.toFixed(
							4
						)} pu {station.worst.assessment.state === 'low'
							? 'below minimum'
							: 'above maximum'}</button
					>{/if}
				<span>{station.unknown} missing values or limits, {station.disconnected} de-energized</span>
			</div>
		</details>
	{/if}
</section>

<style>
	code {
		font: inherit;
	}
	[title] {
		text-decoration: underline dotted;
		text-underline-offset: 2px;
		cursor: help;
	}
	.low {
		color: #2166ac;
		font-weight: 600;
	}
	.high {
		color: #9a5a2c;
		font-weight: 600;
	}
	.operating-panel {
		width: 100%;
		min-width: 0;
		font-family: var(--font-display);
		font-size: var(--fs-label);
		line-height: var(--lh-snug);
	}
	.counts,
	.voltage-key {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2xs) var(--space-lg);
		align-items: center;
	}
	.counts,
	.station-values {
		font-variant-numeric: tabular-nums;
	}
	.voltage-key span {
		font-family: var(--font-mono);
		font-size: var(--fs-control);
		font-variant-numeric: tabular-nums;
	}
	p {
		margin: var(--space-3xs) 0;
	}
	.freshness {
		font-size: var(--fs-control);
		color: var(--text-secondary);
	}
	.warning {
		color: var(--text-accent);
	}
	.violation {
		color: var(--text-danger);
		font-weight: 600;
	}
	.counts {
		margin: var(--space-xs) 0;
	}
	details {
		border-top: 1px solid var(--line);
		padding-top: var(--space-3xs);
		margin-top: var(--space-3xs);
	}
	summary {
		cursor: pointer;
		padding: var(--space-2xs) 0;
		overflow-wrap: anywhere;
	}
	.settings-body,
	.station-values {
		max-height: 24vh;
		overflow: auto;
		padding: var(--space-2xs) var(--space-3xs);
		display: grid;
		gap: var(--space-xs);
	}
	.voltage-key i {
		display: inline-block;
		width: 90px;
		height: 7px;
	}
	label {
		display: flex;
		gap: var(--space-xs);
		align-items: center;
	}
	input {
		width: 64px;
		font: inherit;
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums;
		text-align: right;
		background: var(--paper);
		color: var(--ink);
		border: 1px solid var(--line);
		border-radius: var(--radius-sm);
		padding: var(--space-2xs);
	}
	button {
		text-align: left;
		font: inherit;
		color: var(--ink);
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: var(--radius-sm);
		padding: var(--space-xs);
		cursor: pointer;
	}
	@media (pointer: coarse) {
		summary {
			min-height: 44px;
			align-content: center;
		}
		input,
		button {
			min-height: 44px;
			font-size: 16px;
		}
	}
</style>
