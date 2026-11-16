<script lang="ts">
	import { getController } from '../../context.svelte.js';
	import { cyclesToSeconds, secondsToCycles, type DynEvent } from '../../dynamics/events.js';
	import { WECC240_PRESETS } from '../../dynamics/presets.js';

	const ctrl = getController();
	const dyn = ctrl.dynamics;
	const f0 = $derived(dyn.f0);
	const errors = $derived(dyn.eventErrors);
	let clearingUnit = $state<'cycles' | 'ms'>('cycles');

	const icon = (e: DynEvent) =>
		e.kind === 'bus_fault' || e.kind === 'line_fault' ? '⚡' : e.kind === 'load_change' ? '±' : '✂';

	function clearValue(e: DynEvent & { clearAfter: number }) {
		return clearingUnit === 'cycles'
			? Math.round(secondsToCycles(e.clearAfter, f0) * 100) / 100
			: Math.round(e.clearAfter * 1e5) / 100;
	}
	function setClear(id: string, raw: number) {
		if (!Number.isFinite(raw)) return;
		const s = clearingUnit === 'cycles' ? cyclesToSeconds(raw, f0) : raw / 1000;
		dyn.update(id, { clearAfter: s } as Partial<DynEvent>);
	}
</script>

<section aria-labelledby="dyn-events-h">
	<div class="head">
		<h3 id="dyn-events-h">Events</h3>
		{#if dyn.active?.demo}
			<label class="presets"
				><span>Validated scenarios</span>
				<select
					value=""
					onchange={(e) => {
						if (e.currentTarget.value) dyn.usePreset(e.currentTarget.value);
						e.currentTarget.value = '';
					}}
				>
					<option value="">Choose…</option>
					{#each WECC240_PRESETS as p (p.id)}<option value={p.id}>{p.title}</option>{/each}
				</select></label
			>
		{/if}
	</div>
	{#if dyn.beforePreset}
		<p class="note" data-testid="dyn-preset-settings">
			<span class="chip">Validation settings</span>
		</p>
	{/if}
	{#if dyn.events.length === 0}
		<p class="dim">No events</p>
	{:else}
		<ol class="events">
			{#each dyn.events as e (e.id)}
				{@const err = errors.get(e.id)}
				<li class={{ invalid: !!err }}>
					<span class="icon" aria-hidden="true">{icon(e)}</span>
					<div class="body">
						<p>{dyn.describe(e)}</p>
						<div class="fields">
							<label
								>at <input
									type="number"
									step="0.01"
									min="0"
									value={e.at}
									onchange={(ev) => dyn.update(e.id, { at: Number(ev.currentTarget.value) })}
								/> s</label
							>
							{#if e.kind === 'bus_fault' || e.kind === 'line_fault'}
								<label
									>clear after <input
										type="number"
										step={clearingUnit === 'cycles' ? 0.5 : 5}
										min="0"
										value={clearValue(e)}
										onchange={(ev) => setClear(e.id, Number(ev.currentTarget.value))}
									/>
									<button
										class="unit"
										title="Switch between cycles at {f0} Hz and milliseconds"
										onclick={() => (clearingUnit = clearingUnit === 'cycles' ? 'ms' : 'cycles')}
										>{clearingUnit}</button
									></label
								>
							{/if}
							{#if e.kind === 'bus_fault'}
								<label
									>x <input
										type="number"
										step="0.0001"
										min="0"
										value={e.x}
										onchange={(ev) =>
											dyn.update(e.id, { x: Number(ev.currentTarget.value) } as Partial<DynEvent>)}
									/> pu</label
								>
							{/if}
							{#if e.kind === 'load_change'}
								<label
									>scale <input
										type="number"
										step="0.05"
										min="0"
										value={e.scale}
										onchange={(ev) =>
											dyn.update(e.id, {
												scale: Number(ev.currentTarget.value)
											} as Partial<DynEvent>)}
									/></label
								>
							{/if}
						</div>
						{#if err}<p class="err" role="alert">{err}</p>{/if}
					</div>
					<button
						class="remove"
						aria-label="Remove event: {dyn.describe(e)}"
						onclick={() => dyn.remove(e.id)}>×</button
					>
				</li>
			{/each}
		</ol>
		<button class="link" onclick={() => dyn.clearEvents()}>Clear events</button>
	{/if}
</section>

<style>
	.head {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		gap: 8px;
		flex-wrap: wrap;
	}
	h3 {
		margin: 0;
		font-size: var(--fs-heading);
	}
	.note {
		margin: 6px 0 0;
		font-size: var(--fs-control);
	}
	.chip {
		padding: 1px 8px;
		border-radius: var(--radius-full);
		background: var(--accent-soft);
		color: var(--text-accent);
	}
	.presets {
		display: flex;
		gap: 6px;
		align-items: center;
		font-size: var(--fs-control);
	}
	.events {
		list-style: none;
		margin: 6px 0;
		padding: 0;
		display: grid;
		gap: 6px;
	}
	li {
		display: grid;
		grid-template-columns: 18px 1fr auto;
		gap: 6px;
		padding: 6px;
		border: 1px solid var(--border-default);
		border-radius: var(--radius-sm);
		background: var(--surface-chip);
	}
	li.invalid {
		border-left: 3px solid var(--text-danger);
	}
	.icon {
		font-size: 13px;
	}
	.body p {
		margin: 0;
		line-height: 1.4;
	}
	.fields {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		margin-top: 4px;
		font-size: var(--fs-control);
		color: var(--text-secondary);
	}
	input,
	select {
		width: 5.5em;
		padding: 2px 4px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		background: var(--surface-control);
		color: var(--ink);
		font: inherit;
		font-family: var(--font-mono);
	}
	select {
		width: auto;
		font-family: inherit;
	}
	.unit,
	.remove,
	.link {
		border: none;
		background: none;
		color: var(--text-accent);
		cursor: pointer;
		font: inherit;
		padding: 0 2px;
	}
	.remove {
		font-size: 16px;
		color: var(--text-tertiary);
		align-self: start;
	}
	.err {
		color: var(--text-danger);
		font-size: var(--fs-control);
	}
	.dim {
		color: var(--text-tertiary);
		line-height: 1.45;
	}
</style>
