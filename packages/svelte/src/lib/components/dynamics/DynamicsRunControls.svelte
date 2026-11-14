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
		['power_flow', 'As the power flow solved them'],
		['constant_impedance', 'Constant impedance (Z)'],
		['constant_current', 'Constant current (I)'],
		['constant_power', 'Constant power (P)']
	];
</script>

<section aria-labelledby="dyn-run-h">
	<h3 id="dyn-run-h">Run</h3>
	<div class="grid">
		<span class="lbl" id="dyn-dur">Duration</span>
		<div class="chips" role="group" aria-labelledby="dyn-dur">
			{#each DURATION_CHOICES as d (d)}
				<button aria-pressed={dyn.tEnd === d} onclick={() => (dyn.tEnd = d)}>{d} s</button>
			{/each}
			<input
				type="number"
				min="0.1"
				max={MAX_DURATION}
				step="1"
				aria-label="Duration in seconds"
				value={dyn.tEnd}
				onchange={(e) => {
					const v = Number(e.currentTarget.value);
					if (v > 0 && v <= MAX_DURATION) dyn.tEnd = v;
				}}
			/>
		</div>
		<label class="lbl" for="dyn-step">Step</label>
		<select
			id="dyn-step"
			value={String(dyn.dt)}
			onchange={(e) => (dyn.dt = Number(e.currentTarget.value))}
		>
			{#each STEP_CHOICES as s (s.dt)}<option value={String(s.dt)}>{s.label}</option>{/each}
		</select>
		<label class="lbl" for="dyn-load">Loads</label>
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
		<div class="adv">
			<label
				>Low-voltage load
				<select
					value={o.low_voltage_load?.kind ?? 'break'}
					onchange={(e) =>
						dyn.setOptions({
							...o,
							low_voltage_load:
								e.currentTarget.value === 'exact'
									? { kind: 'exact' }
									: { kind: 'break', v_break: 0.7 }
						})}
					><option value="break">PQBRAK at 0.7 pu (PSS/E)</option><option value="exact"
						>Exact law</option
					></select
				></label
			>
			<label class="check"
				><input
					type="checkbox"
					checked={o.reactive_limits ?? true}
					onchange={(e) => dyn.setOptions({ ...o, reactive_limits: e.currentTarget.checked })}
				/> Reactive limits at initialization</label
			>
			<label class="check"
				><input
					type="checkbox"
					checked={o.renewable_plants ?? true}
					onchange={(e) => dyn.setOptions({ ...o, renewable_plants: e.currentTarget.checked })}
				/> Simulate renewable plants (else held at power-flow output)</label
			>
			<fieldset>
				<legend>Compare with PowerSimulationsDynamics.jl</legend>
				<label class="check"
					><input
						type="checkbox"
						checked={!!o.psid_lv_gain}
						onchange={(e) => dyn.setOptions({ ...o, psid_lv_gain: e.currentTarget.checked })}
					/> Its renewable low-voltage current gain</label
				>
				<label class="check"
					><input
						type="checkbox"
						checked={!!o.psid_converter_ramp}
						onchange={(e) => dyn.setOptions({ ...o, psid_converter_ramp: e.currentTarget.checked })}
					/> Its converter ramp limits</label
				>
				<label class="check"
					><input
						type="checkbox"
						checked={!!o.psid_saturation}
						onchange={(e) => dyn.setOptions({ ...o, psid_saturation: e.currentTarget.checked })}
					/> Its saturation below the knee</label
				>
			</fieldset>
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
			title={dyn.modesBusy ? 'Small-signal modes are being computed' : 'Ctrl/Cmd+Enter'}
			>Run {dyn.tEnd} s simulation</button
		>
		{#if dyn.modesBusy}<p class="dim">
				Small-signal modes are being computed; stop them on the Modes tab to run now.
			</p>{/if}
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
	h3 {
		margin: 0 0 6px;
		font-size: var(--fs-heading);
	}
	.grid {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 6px 10px;
		align-items: center;
		font-size: var(--fs-label);
	}
	.lbl {
		color: var(--text-secondary);
	}
	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}
	.chips button {
		padding: 2px 8px;
		border: 1px solid var(--border-default);
		border-radius: 10px;
		background: var(--surface-chip);
		color: var(--ink);
		font: inherit;
		cursor: pointer;
	}
	.chips button[aria-pressed='true'] {
		border-color: var(--accent);
		box-shadow: var(--inset-active);
	}
	.chips input {
		width: 4.2em;
	}
	select,
	input {
		padding: 3px 5px;
		border: 1px solid var(--line);
		border-radius: 4px;
		background: var(--surface-control);
		color: var(--ink);
		font: inherit;
	}
	details {
		margin: 8px 0;
		font-size: var(--fs-label);
	}
	.adv {
		display: grid;
		gap: 6px;
		margin-top: 6px;
	}
	.adv label {
		display: flex;
		align-items: center;
		gap: 6px;
		justify-content: space-between;
	}
	.adv label.check {
		justify-content: flex-start;
	}
	fieldset {
		border: 1px solid var(--border-default);
		border-radius: var(--radius-sm);
		display: grid;
		gap: 4px;
	}
	.run {
		width: 100%;
		min-height: 36px;
		margin-top: 6px;
		border: 1px solid var(--accent);
		border-radius: var(--radius-sm);
		background: var(--accent);
		color: var(--paper);
		font: inherit;
		font-weight: 600;
		cursor: pointer;
	}
	.run:disabled {
		opacity: 0.5;
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
