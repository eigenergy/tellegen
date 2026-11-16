<script lang="ts">
	import type { RmsLoadModel as LoadModel } from '@tellegen/engine';
	import { tick } from 'svelte';
	import { getController } from '../../context.svelte.js';
	import {
		DURATION_CHOICES,
		MAX_DURATION,
		STEP_CHOICES
	} from '../../dynamics/controller.svelte.js';

	const ctrl = getController();
	const dyn = ctrl.dynamics;
	const ready = $derived(dyn.active?.status === 'ready');
	const o = $derived(dyn.options);
	const pct = $derived(
		dyn.progress ? Math.min(100, (100 * dyn.progress.t) / dyn.progress.tEnd) : 0
	);

	/** Run, then bring the verdict (the payoff) into view: it renders below
	 * these controls, past the panel's fold. */
	async function run() {
		const r = await dyn.run();
		if (!r) return;
		await tick();
		const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
		document
			.querySelector('[data-testid="dyn-verdict"]')
			?.scrollIntoView({ block: 'start', behavior: reduced ? 'auto' : 'smooth' });
	}

	const LOADS: [LoadModel, string][] = [
		['power_flow', 'Power flow'],
		['constant_impedance', 'Impedance (Z)'],
		['constant_current', 'Current (I)'],
		['constant_power', 'Power (P)']
	];
</script>

<section aria-labelledby="dyn-run-h">
	<h3 id="dyn-run-h">Options</h3>
	<div class="setting">
		<label for="dyn-dur">Duration</label>
		<span class="with-unit">
			<input
				id="dyn-dur"
				type="number"
				min="0.1"
				max={MAX_DURATION}
				step="1"
				list="dyn-dur-choices"
				value={dyn.tEnd}
				onchange={(e) => {
					const v = Number(e.currentTarget.value);
					if (v > 0 && v <= MAX_DURATION) dyn.tEnd = v;
					else e.currentTarget.value = String(dyn.tEnd);
				}}
			/>s
			<datalist id="dyn-dur-choices">
				{#each DURATION_CHOICES as d (d)}<option value={d}></option>{/each}
			</datalist>
		</span>
	</div>
	<div class="setting">
		<label for="dyn-step">Step</label>
		<select
			id="dyn-step"
			value={String(dyn.dt)}
			onchange={(e) => (dyn.dt = Number(e.currentTarget.value))}
		>
			{#each STEP_CHOICES as s (s.dt)}<option value={String(s.dt)}>{s.label}</option>{/each}
		</select>
	</div>
	<div class="setting">
		<label for="dyn-load">Loads</label>
		<select
			id="dyn-load"
			value={o.load_model ?? 'power_flow'}
			onchange={(e) => dyn.setOptions({ ...o, load_model: e.currentTarget.value as LoadModel })}
		>
			{#each LOADS as [v, l] (v)}<option value={v}>{l}</option>{/each}
		</select>
	</div>
	<details>
		<summary>Advanced</summary>
		<div class="setting">
			<label for="dyn-lv">Low-voltage load</label>
			<select
				id="dyn-lv"
				value={o.low_voltage_load?.kind ?? 'break'}
				onchange={(e) =>
					dyn.setOptions({
						...o,
						low_voltage_load:
							e.currentTarget.value === 'exact'
								? { kind: 'exact' }
								: { kind: 'break', v_break: 0.7 }
					})}><option value="break">PSS/E</option><option value="exact">Exact</option></select
			>
		</div>
		<div class="setting">
			<label for="dyn-qlim">Reactive limits</label>
			<input
				id="dyn-qlim"
				type="checkbox"
				checked={o.reactive_limits ?? true}
				onchange={(e) => dyn.setOptions({ ...o, reactive_limits: e.currentTarget.checked })}
			/>
		</div>
		<div class="setting">
			<label for="dyn-ren">Simulate renewables</label>
			<input
				id="dyn-ren"
				type="checkbox"
				checked={o.renewable_plants ?? true}
				onchange={(e) => dyn.setOptions({ ...o, renewable_plants: e.currentTarget.checked })}
			/>
		</div>
		<div class="setting" title="As in PowerSimulationsDynamics">
			<label for="dyn-lvg">Low-voltage current gain</label>
			<input
				id="dyn-lvg"
				type="checkbox"
				checked={!!o.psid_lv_gain}
				onchange={(e) => dyn.setOptions({ ...o, psid_lv_gain: e.currentTarget.checked })}
			/>
		</div>
		<div class="setting" title="As in PowerSimulationsDynamics">
			<label for="dyn-ramp">Ramp limits</label>
			<input
				id="dyn-ramp"
				type="checkbox"
				checked={!!o.psid_converter_ramp}
				onchange={(e) => dyn.setOptions({ ...o, psid_converter_ramp: e.currentTarget.checked })}
			/>
		</div>
		<div class="setting" title="As in PowerSimulationsDynamics">
			<label for="dyn-sat">Low saturation</label>
			<input
				id="dyn-sat"
				type="checkbox"
				checked={!!o.psid_saturation}
				onchange={(e) => dyn.setOptions({ ...o, psid_saturation: e.currentTarget.checked })}
			/>
		</div>
	</details>
	{#if dyn.running}
		<div class="progress" role="status">
			<div class="bar" style:width="{pct}%"></div>
			<span>Simulating… {dyn.progress?.t.toFixed(1)} / {dyn.progress?.tEnd} s</span>
			<button class="stop" onclick={() => dyn.stop()}>Stop</button>
		</div>
	{:else}
		<button
			class="run"
			disabled={!ready || dyn.modesBusy}
			onclick={() => void run()}
			title={dyn.modesBusy ? 'Modes running' : 'Ctrl/Cmd+Enter'}>Run {dyn.tEnd} s simulation</button
		>
		{#if dyn.modesBusy}<p class="dim">Modes running</p>{/if}
	{/if}
	{#if dyn.error}<p class="err" role="alert">{dyn.error}</p>{/if}
</section>

<svelte:window
	onkeydown={(e) => {
		if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && ready && !dyn.running && !dyn.modesBusy) {
			e.preventDefault();
			void run();
		}
	}}
/>

<style>
	.run {
		width: 100%;
		min-height: 36px;
		margin-top: 12px;
		border: 1px solid var(--accent);
		border-radius: var(--radius-sm);
		background: var(--accent);
		color: var(--paper);
		font: inherit;
		font-weight: 600;
		cursor: pointer;
	}
	/* Disabled is a flat grey fill with no border, unlike any enabled button. */
	.run:disabled {
		border-color: transparent;
		background: var(--surface-hover);
		color: var(--text-tertiary);
		cursor: not-allowed;
	}
	.progress {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		margin-top: 6px;
		padding: 6px 8px;
		border: 1px solid var(--border-default);
		border-radius: var(--radius-sm);
		overflow: hidden;
		font-size: var(--fs-label);
		font-variant-numeric: tabular-nums;
	}
	.bar {
		position: absolute;
		inset: 0 auto 0 0;
		background: var(--accent-soft);
		transition: width 0.1s linear;
	}
	.progress span,
	.progress button {
		position: relative;
	}
	.stop {
		border: 1px solid var(--text-danger);
		border-radius: var(--radius-sm);
		background: none;
		color: var(--text-danger);
		font: inherit;
		cursor: pointer;
	}
	.err {
		color: var(--text-danger);
		font-size: var(--fs-label);
	}
	.dim {
		margin: 4px 0 0;
		color: var(--text-tertiary);
		font-size: var(--fs-control);
	}
	@media (prefers-reduced-motion: reduce) {
		.bar {
			transition: none;
		}
	}
</style>
