<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import PanelFrame from './PanelFrame.svelte';
	import CalculationSettings from './CalculationSettings.svelte';
	const app = getAppState();
	const ctrl = getController();
</script>

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
		<h3>Display preferences</h3>
		<p>Saved in this browser, independently of calculation results.</p>
		<label
			>Theme<select
				value={app.preferences.theme ?? 'host'}
				onchange={(e) =>
					app.preferences.update({
						theme: e.currentTarget.value === 'host' ? null : e.currentTarget.value
					})}
				><option value="host">Use application default</option><option value="light">Light</option
				><option value="dark">Dark</option></select
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
			>Preferred coloring<select
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
			onclick={() => {
				app.preferences.reset();
				if (ctrl.displayOptions.some((x) => x.mode === 'price')) app.displayMode = 'price';
			}}>Restore display defaults</button
		>
		{#if app.preferences.error}<p role="alert">{app.preferences.error}</p>{/if}
		<p>Camera, selection and panel positions remain in their existing view controls.</p>
	</div>
</PanelFrame>

<style>
	.settings-content {
		padding: 16px;
		font-size: 12px;
	}
	h3 {
		margin: 0 0 10px;
	}
	p {
		line-height: 1.5;
	}
	label {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		margin: 10px 0 4px;
	}
	select {
		padding: 4px 6px;
		border: 1px solid var(--line);
		border-radius: 4px;
		background: var(--surface-control);
		color: var(--ink);
		font: inherit;
		max-width: 55%;
	}
	hr {
		margin: 20px 0;
	}
</style>
