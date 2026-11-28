<script lang="ts">
	/** The Live tab (experimental): opens the live workbench at the bottom,
	 * where the case runs as it goes and disturbances apply the moment they
	 * are picked.
	 * @block Live tab */
	import { getController } from '../../context.svelte.js';

	import LiveDisturbances from './LiveDisturbances.svelte';
	import { SPEEDS, STEPS, WINDOWS } from '../../dynamics/live.svelte.js';
	const dyn = getController().dynamics;
	const live = dyn.live;
</script>

<section class="live" aria-label="Live simulation" data-testid="dyn-live">
	<button
		class="primary"
		disabled={live.running || live.starting || dyn.active?.status !== 'ready' || dyn.running}
		onclick={() => dyn.startLive()}
		data-testid="dyn-live-start"
	>
		{live.starting
			? 'Starting…'
			: live.running
				? 'Live simulation running'
				: live.active
					? 'Resume live simulation'
					: 'Start live simulation'}
	</button>
	<div class="settings">
		<label>
			Pace <select bind:value={live.speed}>
				{#each SPEEDS as s (s)}<option value={s}>
						{Number.isFinite(s) ? `${s}×` : 'Unpaced'}
					</option>{/each}
			</select>
		</label>
		<label>
			Plot window <select bind:value={live.windowS}>
				{#each WINDOWS as w (w)}<option value={w}>{w} s</option>{/each}
			</select>
		</label>
		<label>
			Integration step <select
				value={live.dt}
				onchange={(e) => void live.setStep(Number(e.currentTarget.value))}
			>
				{#each STEPS as s (s)}<option value={s}>{s * 1000} ms</option>{/each}
			</select>
		</label>
	</div>
	{#if live.active}<LiveDisturbances />{/if}
	{#if live.perf}
		<details>
			<summary>Timing</summary>
			<dl>
				<dt>Playback pace</dt>
				<dd>{live.achieved?.toFixed(1)}×</dd>
				<dt>Including worker transfer</dt>
				<dd>{live.perf.roundX.toFixed(1)}× real time</dd>
				<dt>Worker round trip</dt>
				<dd>{live.perf.sliceMs.toFixed(1)} ms</dd>
				{#if Number.isFinite(live.speed)}
					<dt>Clock lag</dt>
					<dd>{live.perf.lagMs.toFixed(0)} ms</dd>
				{/if}
			</dl>
		</details>
	{/if}
	{#if live.error}<p class="err" role="alert">{live.error}</p>{/if}
	{#if live.active && dyn.events.length}
		<h5>Scheduled disturbances</h5>
		<ul class="events">
			{#each dyn.events as e (e.id)}
				<li>
					<span>{dyn.describe(e).split(' at ')[0]}</span>
					<button disabled={!live.running} onclick={() => live.now(e)}>Apply now</button>
				</li>
			{/each}
		</ul>
	{/if}
</section>

<style>
	.live {
		display: grid;
		gap: 8px;
	}
	h5 {
		margin: 0;
	}
	.primary {
		min-height: 36px;
		border: 1px solid var(--accent);
		border-radius: var(--radius-sm);
		background: var(--accent);
		color: var(--paper);
		font: inherit;
		font-weight: 600;
		cursor: pointer;
	}
	.events {
		margin: 0;
		padding: 0;
		list-style: none;
		display: grid;
		gap: 2px;
		font-size: var(--fs-control);
	}
	.events li {
		display: flex;
		justify-content: space-between;
		gap: 6px;
		align-items: center;
	}
	dl {
		display: grid;
		grid-template-columns: 1fr auto;
		gap: 4px;
	}
	dd {
		margin: 0;
		font-variant-numeric: tabular-nums;
	}
	.settings {
		display: grid;
		gap: 6px;
	}
	.settings label {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}
	.err {
		color: var(--text-danger);
		overflow-wrap: anywhere;
	}
</style>
