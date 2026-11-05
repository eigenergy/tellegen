<script lang="ts">
	import { formulationLabel } from '../../format.js';
	import {
		voltageGradient,
		type operatingContext,
		type stationVoltageSummary
	} from '../../diagram/transmission-results.js';
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
		counts: {
			low: number;
			high: number;
			overload: number;
			warning: number;
			atLimit: number;
			voltageUnknown: number;
			loadingUnknown: number;
			inactive: number;
			disconnected: number;
		};
		warning: number;
		onwarning: (value: number) => void;
		station: ({ name: string } & ReturnType<typeof stationVoltageSummary>) | null;
		onselectbus: (id: number) => void;
		/** Whether the area shading shows voltage against the bus limits. */
		shading?: boolean;
		/** Collapsed to the heading and the counts that matter. */
		brief?: boolean;
	} = $props();
</script>

<section class="operating-panel" aria-label="Operating limits">
	<div class="heading">
		<strong>Operating limits</strong><span>{formulationLabel(formulation)}</span
		>{#if context.relaxed}<strong class="relaxation">Relaxation</strong>{/if}
	</div>
	<p class:stale={context.stale} class="freshness" role="status" hidden={brief && !context.stale}>
		{context.freshness} · {context.status} · base case
	</p>
	{#if !brief}{#if context.voltage && voltageAvailable}<div class="voltage-key">
				<span>Voltage</span><span>0.90</span><i style:background={voltageGradient}></i><span
					>1.10 pu</span
				>
			</div>{/if}
		{#if shading}<p class="freshness">
				Shading: near the <span class="low">minimum</span> or
				<span class="high">maximum</span> voltage limit
			</p>{/if}
		<p class="freshness">Loading: {context.basis} · tick at 100%</p>{/if}
	<div class="counts" aria-label="Operating limit counts">
		{#if voltageAvailable}<span class:violation={counts.low + counts.high > 0}
				>↓ {counts.low} low · ↑ {counts.high} high</span
			>
		{:else}<span>{context.voltage ? 'Voltage results unavailable' : 'DC: |V| not solved'}</span
			>{/if}
		<span class:violation={counts.overload > 0}
			>{counts.overload} overload{counts.overload === 1 ? '' : 's'}</span
		>
		<span class:warning={counts.warning + counts.atLimit > 0}
			>{counts.warning} warning · {counts.atLimit} at rating</span
		>
		<span hidden={brief}
			>{context.voltage ? `${counts.voltageUnknown} voltage unknown · ` : ''}{counts.loadingUnknown} loading
			unknown · {counts.inactive} out of service{counts.disconnected
				? ` · ${counts.disconnected} de-energized`
				: ''}</span
		>
	</div>
	<details class="settings" hidden={brief}>
		<summary>Key and settings</summary>
		<div class="settings-body">
			{#if context.voltage}
				<p>
					↓ low · ↑ high, against case limits. Faded: missing voltage or limits. × de-energized.
				</p>{/if}
			<p>
				Loading: {context.basis}. Gauges show 0–125%, with a tick at 100%, and a leader to their
				branch. Values above the gauge range keep their numeric label. A heavy amber outline means
				at rating; a filled red gauge means overloaded.
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
				Grey below {warning}% · amber {warning}–100% · red above 100%. At rating does not mean
				economically binding.
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
						· {station.worst.assessment.state === 'low' ? '↓' : '↑'} bus {station.worst.bus}</span
					>{/if}</summary
			>
			<div class="station-values" aria-label="Station voltage range">
				{#if station.min}<button onclick={() => onselectbus(station.min!.bus)}
						>Minimum: bus {station.min.bus} · {station.min.value.toFixed(4)} pu</button
					>{/if}
				{#if station.max}<button onclick={() => onselectbus(station.max!.bus)}
						>Maximum: bus {station.max.bus} · {station.max.value.toFixed(4)} pu</button
					>{/if}
				{#if station.worst}<button class="violation" onclick={() => onselectbus(station.worst!.bus)}
						>Worst violation: bus {station.worst.bus} · {station.worst.assessment.exceedance.toFixed(
							4
						)} pu {station.worst.assessment.state === 'low'
							? 'below minimum'
							: 'above maximum'}</button
					>{/if}
				<span>{station.unknown} missing values or limits · {station.disconnected} de-energized</span
				>
			</div>
		</details>
	{/if}
</section>

<style>
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
		font: 11px var(--font-mono);
	}
	.heading,
	.counts,
	.voltage-key {
		display: flex;
		flex-wrap: wrap;
		gap: 4px 12px;
		align-items: center;
	}
	.heading strong {
		color: var(--ink);
	}
	p {
		margin: 3px 0;
	}
	.freshness {
		font-size: 10px;
	}
	.stale,
	.warning,
	.relaxation {
		color: #925b05;
	}
	.violation {
		color: #be2024;
		font-weight: 600;
	}
	.counts {
		margin: 6px 0;
	}
	details {
		border-top: 1px solid var(--line);
		padding-top: 3px;
		margin-top: 3px;
	}
	summary {
		cursor: pointer;
		padding: 4px 0;
		overflow-wrap: anywhere;
	}
	.settings-body,
	.station-values {
		max-height: 24vh;
		overflow: auto;
		padding: 4px 2px;
		display: grid;
		gap: 6px;
	}
	.voltage-key i {
		display: inline-block;
		width: 90px;
		height: 7px;
	}
	label {
		display: flex;
		gap: 6px;
		align-items: center;
	}
	input {
		width: 64px;
		font: inherit;
		background: var(--paper);
		color: var(--ink);
		border: 1px solid var(--line);
		border-radius: 3px;
		padding: 5px;
	}
	button {
		text-align: left;
		font: inherit;
		color: var(--ink);
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: 3px;
		padding: 6px;
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
