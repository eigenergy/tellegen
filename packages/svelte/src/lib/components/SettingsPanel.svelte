<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { getPanelLayout } from '../panels.svelte.js';
	import PanelFrame from './PanelFrame.svelte';
	import CalculationSettings from './CalculationSettings.svelte';
	const app = getAppState();
	const ctrl = getController();
	const layout = getPanelLayout();
	const support = $derived(ctrl.dynamics.support);
	let calculation = $state<ReturnType<typeof CalculationSettings>>();
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
		<CalculationSettings bind:this={calculation} />

		<h3>Display</h3>
		<div class="row">
			<label for="setting-theme">Theme</label>
			<select
				id="setting-theme"
				value={app.preferences.theme ?? 'host'}
				onchange={(e) =>
					app.preferences.update({
						theme: e.currentTarget.value === 'host' ? null : e.currentTarget.value
					})}
				><option value="host">System</option><option value="light">Light</option><option
					value="dark">Dark</option
				></select
			>
		</div>
		<div class="row">
			<label for="setting-bus-sizing">Bus sizing</label>
			<select
				id="setting-bus-sizing"
				value={app.busSizing}
				onchange={(e) => app.preferences.update({ busSizing: e.currentTarget.value })}
				><option value="power">By power</option><option value="uniform">Uniform</option></select
			>
		</div>
		<div class="row">
			<label for="setting-coloring">Bus colour</label>
			<select
				id="setting-coloring"
				value={app.preferences.coloring}
				onchange={(e) => {
					app.preferences.update({ coloring: e.currentTarget.value });
					if (ctrl.displayOptions.some((x) => x.mode === e.currentTarget.value))
						app.displayMode = e.currentTarget.value as typeof app.displayMode;
				}}
				><option value="price">Price</option><option value="voltage">Voltage</option><option
					value="angle">Angle</option
				><option value="demand">Demand</option></select
			>
		</div>
		{#if app.preferences.error}<p class="error" role="alert">{app.preferences.error}</p>{/if}

		<h3>Experimental</h3>
		<div class="row">
			<label for="setting-experimental">Experimental features</label>
			<input
				id="setting-experimental"
				type="checkbox"
				class="switch"
				checked={app.preferences.experimental}
				disabled={support !== null && !support.rms}
				title={support !== null && !support.rms ? 'Not in this build' : undefined}
				onchange={(e) => app.preferences.update({ experimental: e.currentTarget.checked })}
			/>
		</div>

		<div class="footer">
			<button
				class="secondary"
				onclick={() => {
					app.preferences.reset();
					if (ctrl.displayOptions.some((x) => x.mode === 'price')) app.displayMode = 'price';
					calculation?.restoreDefaults();
				}}>Restore defaults</button
			>
		</div>
	</div>
</PanelFrame>

<style>
	.settings-content {
		padding: 12px 16px 16px;
		font-size: var(--fs-body);
	}
	h3 {
		margin: 16px 0 2px;
		font-size: var(--fs-label);
		font-weight: 600;
		color: var(--text-secondary);
	}
	/* One setting per row: the name on the left, its control on the right. */
	.row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		min-height: 34px;
		border-bottom: 1px solid var(--line);
	}
	select {
		flex: none;
		width: var(--settings-control-width, 150px);
		box-sizing: border-box;
		min-height: 28px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		background-color: var(--surface-control);
		color: var(--ink);
		font: inherit;
	}
	input.switch {
		width: 16px;
		height: 16px;
		margin: 0;
		accent-color: var(--accent);
	}
	.error {
		margin: 4px 0;
		color: var(--text-danger);
	}
	.footer {
		display: flex;
		justify-content: flex-end;
		margin-top: 16px;
	}
	.secondary {
		min-height: 28px;
		padding: 4px 10px;
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
	select:focus-visible,
	input:focus-visible,
	.secondary:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
	}
	@media (pointer: coarse) {
		.row {
			min-height: 44px;
		}
		.secondary,
		select {
			min-height: 36px;
			font-size: 16px;
		}
	}
</style>
