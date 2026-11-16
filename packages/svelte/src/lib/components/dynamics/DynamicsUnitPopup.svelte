<script lang="ts">
	/** The unit editor opened from a unit's glyph on the map (experimental),
	 * beside the clicked point and movable by its header. It also keeps the
	 * unit catalog in step with the active case, so the glyphs are there
	 * whether or not the Dynamics panel is open. */
	import { getAppState, getController } from '../../context.svelte.js';
	import { unitKey, unitSummary } from '../../dynamics/units.svelte.js';
	import DynamicsElementActions from './DynamicsElementActions.svelte';
	import DynamicsUnitEditor from './DynamicsUnitEditor.svelte';

	const app = getAppState();
	const dyn = getController().dynamics;

	$effect(() => {
		const a = dyn.active;
		const on = app.preferences.experimental && !!dyn.support?.experimental && a?.status !== 'opening';
		dyn.catalog.sync(
			on && a ? app.activeLocal?.id ?? null : null,
			app.activeLocal?.studyInputJson,
			on && a ? a.text : null
		);
	});
	$effect(() => {
		void dyn.probe();
	});

	const at = $derived(dyn.unitPopup);
	const units = $derived(at ? dyn.catalog.units.filter((u) => u.bus === at.bus) : []);
	let key = $state<string | null>(null);
	/** A bus click shows the bus's actions until a unit is picked. */
	let showBus = $state(false);
	let pos = $state<[number, number]>([0, 0]);
	const WIDTH = 420;
	$effect(() => {
		if (!at) return;
		showBus = at.kind === 'bus';
		key = at.kind === 'bus' ? null : `${at.bus}|${at.machineId}`;
		const vw = innerWidth;
		const vh = innerHeight;
		pos = [
			Math.max(8, Math.min(at.x > vw / 2 ? at.x - WIDTH - 18 : at.x + 18, vw - WIDTH - 8)),
			Math.max(56, Math.min(at.y - 40, vh - 480))
		];
	});
	let move: [number, number] | null = null;
</script>

<svelte:window
	onpointermove={(e) => move && (pos = [e.clientX - move[0], e.clientY - move[1]])}
	onpointerup={() => (move = null)}
	onkeydown={(e) =>
		e.key === 'Escape' && !(e.target instanceof HTMLInputElement) && (dyn.unitPopup = null)}
/>

{#if at && app.mode === 'dynamics'}
	<div
		class="popup"
		style:left={`${pos[0]}px`}
		style:top={`${pos[1]}px`}
		style:width={`${WIDTH}px`}
		role="dialog"
		aria-label={`Generator ${at.bus}`}
		data-testid="dyn-unit-popup"
	>
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<header onpointerdown={(e) => (move = [e.clientX - pos[0], e.clientY - pos[1]])}>
			<strong>{showBus ? 'Bus' : 'Generator'} {at.bus}</strong>
			{#if !showBus && units.length > 1}
				<select bind:value={key} onpointerdown={(e) => e.stopPropagation()} aria-label="unit">
					{#each units as u (unitKey(u))}<option value={unitKey(u)}>unit '{u.machine_id.trim()}'</option
						>{/each}
				</select>
			{/if}
			<span class="grow"></span>
			<button
				class="x"
				onpointerdown={(e) => e.stopPropagation()}
				onclick={() => (dyn.unitPopup = null)}
				aria-label="Close">✕</button
			>
		</header>
		<div class="body">
			{#if showBus}
				<DynamicsElementActions />
				{#if units.length}
					<h5>Edit a unit's models</h5>
					<ul class="units">
						{#each units as u (unitKey(u))}
							<li>
								<button
									class="link"
									onclick={() => {
										key = unitKey(u);
										showBus = false;
									}}>unit '{u.machine_id.trim()}'</button
								>
								<span class="dim">{unitSummary(u)}</span>
							</li>
						{/each}
					</ul>
				{/if}
			{:else if key}
				<DynamicsUnitEditor unitKey={key} />
			{/if}
		</div>
	</div>
{/if}

<style>
	.popup {
		position: fixed;
		z-index: 40;
		max-height: calc(100vh - 80px);
		display: flex;
		flex-direction: column;
		background: var(--surface-menu);
		border: 1px solid var(--border-default);
		border-radius: 8px;
		box-shadow: 0 8px 28px rgb(0 0 0 / 0.22);
		color: var(--text-primary);
		font-size: var(--fs-label);
	}
	header {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 6px 10px;
		border-bottom: 1px solid var(--border-default);
		cursor: move;
	}
	.grow {
		flex: 1;
	}
	.x {
		border: none;
		background: none;
		color: var(--text-secondary);
		cursor: pointer;
		font: inherit;
	}
	.body {
		padding: 10px;
		overflow: auto;
	}
	h5 {
		margin: 8px 0 2px;
	}
	.units {
		margin: 0;
		padding: 0;
		list-style: none;
		font-size: var(--fs-control);
	}
	.units li {
		display: flex;
		gap: 8px;
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
	.dim {
		color: var(--text-tertiary);
	}
</style>
