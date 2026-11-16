<script lang="ts">
	import { getController } from '../../context.svelte.js';
	import { cyclesToSeconds, secondsToCycles, type DynEvent } from '../../dynamics/events.js';
	import { WECC240_PRESETS } from '../../dynamics/presets.js';

	const ctrl = getController();
	const dyn = ctrl.dynamics;
	const f0 = $derived(dyn.f0);
	const errors = $derived(dyn.eventErrors);

	const cycles = (e: DynEvent & { clearAfter: number }) =>
		Math.round(secondsToCycles(e.clearAfter, f0) * 100) / 100;
	function setClear(id: string, raw: number) {
		if (!Number.isFinite(raw)) return;
		dyn.update(id, { clearAfter: cyclesToSeconds(raw, f0) } as Partial<DynEvent>);
	}
	const num = (v: string) => Number(v);
</script>

<section aria-labelledby="dyn-events-h">
	<h3 id="dyn-events-h">Events</h3>
	{#if dyn.active?.demo}
		<div class="setting">
			<label for="dyn-scenario">Scenario</label>
			<select
				id="dyn-scenario"
				value=""
				onchange={(e) => {
					if (e.currentTarget.value) dyn.usePreset(e.currentTarget.value);
					e.currentTarget.value = '';
				}}
			>
				<option value="">Choose…</option>
				{#each WECC240_PRESETS as p (p.id)}<option value={p.id}>{p.title}</option>{/each}
			</select>
		</div>
	{/if}
	{#if dyn.events.length === 0}
		<div class="setting"><span>Event</span><span class="value">None</span></div>
	{:else}
		<ol class="events">
			{#each dyn.events as e (e.id)}
				{@const err = errors.get(e.id)}
				{@const name = dyn.describe(e)}
				<li class={{ invalid: !!err }} aria-label={name}>
					<div class="title">
						<span>{name}</span>
						<button
							class="remove"
							aria-label="Remove {name}"
							title="Remove"
							onclick={() => dyn.remove(e.id)}>×</button
						>
					</div>
					<div class="setting">
						<label for="dyn-at-{e.id}">Start</label>
						<span class="with-unit">
							<input
								id="dyn-at-{e.id}"
								type="number"
								step="0.01"
								min="0"
								value={e.at}
								onchange={(ev) => dyn.update(e.id, { at: num(ev.currentTarget.value) })}
							/>s</span
						>
					</div>
					{#if e.kind === 'bus_fault' || e.kind === 'line_fault'}
						<div class="setting">
							<label for="dyn-clear-{e.id}">Clear after</label>
							<span class="with-unit">
								<input
									id="dyn-clear-{e.id}"
									type="number"
									step="0.5"
									min="0"
									value={cycles(e)}
									title="{Math.round(e.clearAfter * 1000)} ms"
									onchange={(ev) => setClear(e.id, num(ev.currentTarget.value))}
								/>cycles</span
							>
						</div>
					{/if}
					{#if e.kind === 'bus_fault'}
						<div class="setting">
							<label for="dyn-x-{e.id}">Reactance</label>
							<span class="with-unit">
								<input
									id="dyn-x-{e.id}"
									type="number"
									step="0.0001"
									min="0"
									value={e.x}
									onchange={(ev) =>
										dyn.update(e.id, { x: num(ev.currentTarget.value) } as Partial<DynEvent>)}
								/>pu</span
							>
						</div>
					{/if}
					{#if e.kind === 'load_change'}
						<div class="setting">
							<label for="dyn-scale-{e.id}">Load scale</label>
							<span class="with-unit">
								<input
									id="dyn-scale-{e.id}"
									type="number"
									step="0.05"
									min="0"
									value={e.scale}
									onchange={(ev) =>
										dyn.update(e.id, {
											scale: num(ev.currentTarget.value)
										} as Partial<DynEvent>)}
								/>×</span
							>
						</div>
					{/if}
					{#if err}<p class="err" role="alert">{err}</p>{/if}
				</li>
			{/each}
		</ol>
		{#if dyn.events.length > 1}
			<button class="text" onclick={() => dyn.clearEvents()}>Remove all</button>
		{/if}
	{/if}
</section>

<style>
	.events {
		display: grid;
		gap: 8px;
		margin: 8px 0 0;
		padding: 0;
		list-style: none;
	}
	li {
		padding: 0 10px 2px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		background: var(--surface-panel);
	}
	li.invalid {
		border-color: var(--red);
	}
	li .setting:last-of-type {
		border-bottom: 0;
	}
	.title {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		min-height: 34px;
		border-bottom: 1px solid var(--line);
		font-weight: 600;
	}
	.remove {
		width: 24px;
		height: 24px;
		padding: 0;
		border: 0;
		border-radius: var(--radius-control);
		background: none;
		color: var(--text-secondary);
		font: inherit;
		font-size: 16px;
		line-height: 1;
		cursor: pointer;
	}
	.remove:hover {
		background: var(--surface-hover);
		color: var(--text-primary);
	}
	.err {
		margin: 0 0 6px;
		color: var(--text-danger);
		font-size: var(--fs-label);
	}
	.text {
		margin-top: 6px;
		padding: 2px 0;
		border: 0;
		background: none;
		color: var(--text-accent);
		font: inherit;
		cursor: pointer;
	}
	.text:hover {
		text-decoration: underline;
	}
</style>
