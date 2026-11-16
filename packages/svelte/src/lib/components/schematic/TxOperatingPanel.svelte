<script lang="ts">
	import type { operatingExceptions } from '../../diagram/transmission-inspection.js';
	import { formulationLabel } from '../../format.js';
	import {
		voltageGradient,
		type operatingContext,
		type stationVoltageSummary
	} from '../../diagram/transmission-results.js';
	import { operatingLead } from '../../diagram/transmission-explain.js';
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

<section class="operating-panel" aria-label="Limits">
	<TxPanelHead title="Limits" {context} {lead} view="operating" />
	{#if !brief}
		{#if context.voltage && voltageAvailable}<div class="voltage-key">
				<span>Voltage</span><span>0.90</span><i style:background={voltageGradient}></i><span
					>1.10 pu</span
				>
			</div>{/if}
		<details class="settings">
			<summary>Key</summary>
			<ul class="key">
				<li><i class="swatch normal"></i>Below {warning}%</li>
				<li><i class="swatch near"></i>Near limit</li>
				<li><i class="swatch over"></i>At limit</li>
				<li><i class="swatch unrated"></i>Unrated</li>
				<li><i class="swatch dashed"></i>Out of service</li>
				{#if context.voltage}
					<li><b class="low">↓</b>Low voltage</li>
					<li><b class="high">↑</b>High voltage</li>
					<li><b>×</b>De-energized</li>
				{/if}
			</ul>
			<label
				>Warn at (%) <input
					aria-label="Warn at (%)"
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
				/></label
			>
		</details>
	{/if}
	{#if station && context.voltage && !brief}
		<details class="station">
			<summary
				><strong>{station.name}</strong>
				{#if station.min && station.max}{station.min.value.toFixed(4)}–{station.max.value.toFixed(
						4
					)} pu{:else}—{/if}
				{#if station.worst}<span class="violation">
						{station.worst.assessment.state === 'low' ? '↓' : '↑'} bus {station.worst.bus}</span
					>{/if}</summary
			>
			<dl class="station-values" aria-label="Station voltage range">
				{#if station.min}<div>
						<dt>Min</dt>
						<dd>
							<button onclick={() => onselectbus(station.min!.bus)}
								>{station.min.value.toFixed(4)} pu, bus {station.min.bus}</button
							>
						</dd>
					</div>{/if}
				{#if station.max}<div>
						<dt>Max</dt>
						<dd>
							<button onclick={() => onselectbus(station.max!.bus)}
								>{station.max.value.toFixed(4)} pu, bus {station.max.bus}</button
							>
						</dd>
					</div>{/if}
				{#if station.worst}<div>
						<dt>Worst</dt>
						<dd>
							<button class="violation" onclick={() => onselectbus(station.worst!.bus)}
								>{station.worst.assessment.state === 'low'
									? '−'
									: '+'}{station.worst.assessment.exceedance.toFixed(4)} pu, bus {station.worst
									.bus}</button
							>
						</dd>
					</div>{/if}
			</dl>
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
		font-family: var(--font-display);
		font-size: var(--fs-label);
		line-height: var(--lh-snug);
	}
	.voltage-key {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2xs) var(--space-lg);
		align-items: center;
	}
	.station-values {
		font-variant-numeric: tabular-nums;
		margin: 0;
	}
	.station-values div {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-sm);
	}
	dt {
		color: var(--text-secondary);
	}
	dd {
		margin: 0;
	}
	.key {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(110px, 1fr));
		gap: var(--space-2xs) var(--space-sm);
		margin: var(--space-2xs) 0 var(--space-xs);
		padding: 0;
		list-style: none;
	}
	.key li {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.key b {
		width: 18px;
		text-align: center;
	}
	.swatch {
		width: 18px;
		height: 3px;
		border-radius: 2px;
	}
	.swatch.normal {
		background: rgb(158 153 144);
	}
	.swatch.near {
		background: var(--amber);
	}
	.swatch.over {
		background: var(--red);
	}
	.swatch.unrated {
		background: rgb(155 150 140 / 0.45);
	}
	.swatch.dashed {
		height: 0;
		border-top: 2px dashed rgb(158 153 144);
		border-radius: 0;
	}
	.voltage-key span {
		font-family: var(--font-mono);
		font-size: var(--fs-control);
		font-variant-numeric: tabular-nums;
	}
	.violation {
		color: var(--text-danger);
		font-weight: 600;
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
