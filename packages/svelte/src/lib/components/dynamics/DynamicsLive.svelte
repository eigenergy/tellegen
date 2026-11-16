<script lang="ts">
	/** The Live tab (experimental): opens the live workbench at the bottom,
	 * where the case runs as it goes and disturbances apply the moment they
	 * are picked. */
	import { getAppState, getController } from '../../context.svelte.js';

	const app = getAppState();
	const dyn = getController().dynamics;
	const live = dyn.live;

	async function openAndStart() {
		const a = dyn.active;
		const l = app.activeLocal;
		app.mode = 'dynamics';
		live.open = true;
		if (!a?.inspection || !l?.studyInputJson || live.running) return;
		await live.start(l.id, {
			moduleJson: l.studyInputJson,
			dynText: dyn.dynText(a),
			inspection: a.inspection,
			loadModel: dyn.options.load_model ?? 'power_flow'
		});
	}
</script>

<section class="live" aria-label="Live simulation" data-testid="dyn-live">
	<p>
		Run the case as it goes, paced to the clock, and apply faults, trips, load steps and reference
		changes the moment you pick them. Plots of any bus, unit or line below the map.
		<span class="pill-experimental">Experimental</span>
	</p>
	<button
		class="primary"
		disabled={live.starting || dyn.active?.status !== 'ready' || dyn.running}
		onclick={openAndStart}
		data-testid="dyn-live-start"
		>{live.starting
			? 'Starting…'
			: live.running
				? 'Live is running'
				: live.active
					? '▶ Resume live'
					: '▶ Start live'}</button
	>
	<p class="dim">
		While it runs, click a bus on the map to fault it or step its load, or a G or I chip to edit that
		unit.
	</p>
	{#if live.error}<p class="err" role="alert">{live.error}</p>{/if}
	{#if live.active && dyn.events.length}
		<h5>Events of the list, now</h5>
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
	p,
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
	.dim {
		color: var(--text-tertiary);
	}
	.err {
		color: var(--text-danger);
		overflow-wrap: anywhere;
	}
</style>
