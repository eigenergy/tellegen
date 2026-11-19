<script lang="ts">
	/** Frequency responses of the linearized case (experimental): from a
	 * unit's voltage or power reference to a unit's speed, angle, field
	 * voltage, power or terminal voltage, as Bode magnitude and phase. The
	 * curves are the engine's (`linear_analysis` with `bode`).
	 * @block Bode plots */
	import type { RmsBodeCurve, RmsBodeSpec } from '@tellegen/engine';
	import { getController } from '../../context.svelte.js';
	import ZoomChart from './ZoomChart.svelte';

	const dyn = getController().dynamics;
	const ins = $derived(dyn.active?.inspection ?? null);
	const units = $derived(
		(ins?.generators ?? []).map((g) => ({ bus: g.bus, id: g.machine_id.trim(), models: g.models }))
	);
	const unitKey = (u: { bus: number; id: string }) => `${u.bus}|${u.id}`;

	let inputUnit = $state('');
	let outputUnit = $state('');
	let input = $state<RmsBodeSpec['input']>('v_ref');
	let output = $state<RmsBodeSpec['output']>('vm');
	let fMin = $state(0.01);
	let fMax = $state(10);
	let curves = $state.raw<RmsBodeCurve[]>([]);
	let busy = $state(false);
	let xDomain = $state<[number, number] | null>(null);
	let hoverX = $state<number | null>(null);
	let error = $state<string | null>(null);
	$effect(() => {
		if (units.length && !units.some((u) => unitKey(u) === inputUnit)) inputUnit = unitKey(units[0]);
		if (units.length && !units.some((u) => unitKey(u) === outputUnit)) outputUnit = inputUnit;
	});

	async function add() {
		const [ib, iid] = inputUnit.split('|');
		const [ob, oid] = outputUnit.split('|');
		busy = true;
		error = null;
		try {
			const r = await dyn.linearAnalysis({
				participation_top: 0,
				f_min: fMin,
				f_max: fMax,
				points: 200,
				bode: [
					{
						input_bus: Number(ib),
						input_machine: iid,
						input,
						output,
						output_bus: Number(ob),
						output_machine: oid
					}
				]
			});
			curves = [...curves, ...r.bode].slice(-6);
		} catch (e) {
			error = (e as Error).message;
		} finally {
			busy = false;
		}
	}

</script>

<section class="bode dyn-form" aria-label="Frequency response" data-testid="dyn-bode">
	<h4>Frequency response <span class="pill-experimental">Experimental</span></h4>
	<div class="form">
		<label
			><span class="cap">From</span><select bind:value={inputUnit}
				>{#each units as u (unitKey(u))}<option value={unitKey(u)}
						>{u.bus}{u.id === '1' ? '' : ` '${u.id}'`} · {u.models
							.filter((m) => m !== 'none')
							.join(' ')}</option
					>{/each}</select
			></label
		>
		<label
			><span class="cap">Input</span><select bind:value={input}
				><option value="v_ref">voltage reference</option><option value="p_ref"
					>power reference</option
				></select
			></label
		>
		<label
			><span class="cap">To</span><select bind:value={outputUnit}
				>{#each units as u (unitKey(u))}<option value={unitKey(u)}
						>{u.bus}{u.id === '1' ? '' : ` '${u.id}'`}</option
					>{/each}</select
			></label
		>
		<label
			><span class="cap">Output</span><select bind:value={output}
				><option value="vm">terminal voltage</option><option value="omega">speed</option><option
					value="delta">rotor angle</option
				><option value="pe">electrical power</option><option value="efd">field voltage</option
				></select
			></label
		>
		<label><span class="cap">f from [Hz]</span><input type="number" min="0.001" step="any" bind:value={fMin} /></label>
		<label><span class="cap">to [Hz]</span><input type="number" min="0.01" step="any" bind:value={fMax} /></label>
		<button disabled={busy || !units.length || dyn.running} onclick={add}
			>{busy ? 'Computing…' : 'Add response'}</button
		>
	</div>
	{#if error}<p class="err">{error}</p>{/if}
	{#if curves.length}
		<ul class="legend">
			{#each curves as c, i (i)}
				<li>
					<span class="sw" style:background={`var(--series-${(i % 6) + 1})`}></span>{c.label}
					<button class="link" onclick={() => (curves = curves.filter((_, j) => j !== i))}
						>remove</button
					>
				</li>
			{/each}
		</ul>
		<ZoomChart
			series={curves.map((c, i) => ({ key: `${i}`, label: c.label, x: c.freq_hz, y: c.mag_db, slot: i + 1 }))}
			title="Magnitude"
			unit="dB"
			logX
			xLabel="frequency [Hz]"
			digits={2}
			height={170}
			bind:xDomain
			bind:hoverX
		/>
		<ZoomChart
			series={curves.map((c, i) => ({ key: `${i}`, label: c.label, x: c.freq_hz, y: c.phase_deg, slot: i + 1 }))}
			title="Phase"
			unit="deg"
			logX
			xLabel="frequency [Hz]"
			digits={1}
			height={170}
			bind:xDomain
			bind:hoverX
		/>
	{/if}
</section>

<style>
	.bode {
		display: grid;
		gap: 6px;		grid-template-columns: minmax(0, 1fr);
		min-width: 0;
	}
	h4 {
		margin: 0;
	}
	.form {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 4px 8px;
		font-size: var(--fs-control);
	}
	.form label {
		display: grid;
		gap: 2px;
	}
	.form select,
	.form input {
		width: 100%;
	}
	.cap {
		color: var(--text-secondary);
		font-size: var(--fs-meta);
	}
	.form button {
		grid-column: 1 / -1;
	}
	.legend {
		margin: 0;
		padding: 0;
		list-style: none;
		font-size: var(--fs-control);
	}
	.legend li {
		display: flex;
		gap: 6px;
		align-items: center;
	}
	.sw {
		width: 10px;
		height: 3px;
		border-radius: 2px;
	}
	.link {
		border: none;
		background: none;
		padding: 0;
		color: var(--text-accent);
		cursor: pointer;
		font: inherit;
		text-decoration: underline;
	}
	.err {
		color: var(--text-danger);
		overflow-wrap: anywhere;
	}
</style>
