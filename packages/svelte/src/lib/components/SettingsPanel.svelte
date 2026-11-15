<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { getPanelLayout } from '../panels.svelte.js';
	import PanelFrame from './PanelFrame.svelte';
	import CalculationSettings from './CalculationSettings.svelte';
	const app = getAppState();
	const ctrl = getController();
	const layout = getPanelLayout();
	const support = $derived(ctrl.dynamics.support);
	$effect(() => {
		if (app.settingsOpen) void ctrl.dynamics.probe();
	});

	/** Escape closes Settings when it has focus, or when nothing else does. A menu,
	 * dialog or select that handles Escape first marks the event, and wins. */
	function escape(event: KeyboardEvent) {
		if (event.key !== 'Escape' || event.defaultPrevented || !app.settingsOpen) return;
		const panel = layout.panels.find((candidate) => candidate.id === 'settings');
		if (!panel || !layout.shown(panel)) return;
		const active = document.activeElement;
		const inside = !!active?.closest('[data-panel="settings"]');
		if (!inside && active && active !== document.body) return;
		event.preventDefault();
		layout.close(panel);
		const opener = layout.opener(panel);
		if (opener?.isConnected) opener.focus();
	}
</script>

<svelte:window onkeydown={escape} />

<PanelFrame
	id="settings"
	title="Settings"
	side="right"
	order={35}
	width={380}
	bind:open={app.settingsOpen}
>
	<div class="settings-content" role="region" aria-label="Common settings">
		<CalculationSettings />
		<hr />
		<h3>Display</h3>
		<p class="intro">Saved in this browser. They do not change any result.</p>
		<label
			>Theme<select
				value={app.preferences.theme ?? 'host'}
				onchange={(e) =>
					app.preferences.update({
						theme: e.currentTarget.value === 'host' ? null : e.currentTarget.value
					})}
				><option value="host">Match the system</option><option value="light">Light</option><option
					value="dark">Dark</option
				></select
			></label
		>
		<label
			>Bus sizing<select
				value={app.busSizing}
				onchange={(e) => app.preferences.update({ busSizing: e.currentTarget.value })}
				><option value="power">By power</option><option value="uniform">Uniform</option></select
			></label
		>
		<label
			>Colour buses by<select
				value={app.preferences.coloring}
				onchange={(e) => {
					app.preferences.update({ coloring: e.currentTarget.value });
					if (ctrl.displayOptions.some((x) => x.mode === e.currentTarget.value))
						app.displayMode = e.currentTarget.value as typeof app.displayMode;
				}}
				><option value="price">Price</option><option value="voltage">Voltage</option><option
					value="angle">Angle</option
				><option value="demand">Demand</option></select
			></label
		>
		<button
			class="secondary"
			onclick={() => {
				app.preferences.reset();
				if (ctrl.displayOptions.some((x) => x.mode === 'price')) app.displayMode = 'price';
			}}>Restore display defaults</button
		>
		{#if app.preferences.error}<p role="alert">{app.preferences.error}</p>{/if}
		<hr />
		<label class="check"
			><input
				type="checkbox"
				checked={app.preferences.experimental}
				disabled={support !== null && !support.rms}
				onchange={(e) => app.preferences.update({ experimental: e.currentTarget.checked })}
			/>Experimental features</label
		>
		<p class="intro">
			Adds dynamics (RMS) simulation of faults and trips. Its results are still being checked.
		</p>
		{#if support !== null && !support.rms}
			<p class="intro">This engine build does not include dynamics simulation.</p>
		{/if}
	</div>
</PanelFrame>

<style>
	.settings-content {
		padding: 16px;
		font-size: var(--fs-body);
	}
	h3 {
		margin: 0 0 6px;
		font-size: var(--fs-title);
		font-weight: 600;
	}
	p {
		line-height: 1.5;
	}
	.intro {
		margin: 0 0 8px;
		color: var(--text-secondary);
	}
	label {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		margin: 10px 0 4px;
	}
	select {
		max-width: 55%;
	}
	hr {
		margin: 20px 0;
		border: 0;
		border-top: 1px solid var(--line);
	}
	label.check {
		justify-content: flex-start;
		gap: 8px;
	}
	.secondary {
		margin-top: 12px;
		min-height: 28px;
		padding: 5px 10px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		background: var(--surface-control);
		color: var(--ink);
		font: inherit;
		cursor: pointer;
	}
	.secondary:hover {
		border-color: var(--border-strong);
	}
	@media (pointer: coarse) {
		.secondary,
		select {
			min-height: 44px;
		}
	}
</style>
