<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { fmt } from '../format.js';
	import type { LocalCase } from '../state.svelte.js';

	const app = getAppState();
	const ctrl = getController();

	// Powerio writer tokens the committed state exports to; label carries the extension.
	const EXPORT_FORMATS = [
		{ token: 'matpower', label: 'MATPOWER (.m)' },
		{ token: 'psse', label: 'PSS/E (.raw)' },
		{ token: 'powermodels-json', label: 'PowerModels (.json)' },
		{ token: 'pandapower-json', label: 'pandapower (.json)' },
		{ token: 'powerio-json', label: 'PowerIO (.json)' }
	];

	let busy = $state(false);
	let exportOpen = $state(false);
	let exportWarnings = $state<string[]>([]);

	async function saveStudy(lc: LocalCase) {
		busy = true;
		exportWarnings = [];
		try {
			await ctrl.saveStudyPackage(lc);
		} finally {
			busy = false;
		}
	}

	async function exportStudy(lc: LocalCase, token: string) {
		busy = true;
		try {
			exportWarnings = await ctrl.exportStudyAs(lc, token);
			exportOpen = false;
		} finally {
			busy = false;
		}
	}

	async function downloadLayout(lc: LocalCase) {
		busy = true;
		try {
			await ctrl.downloadGeoLayer(lc);
		} finally {
			busy = false;
		}
	}
</script>

{#if app.activeLocal}
	{@const lc = app.activeLocal}
	<h2>{lc.label} <span class="region mono">via {lc.fileName}</span></h2>
	{#if lc.substations}
		<dl class="mono">
			<div>
				<dt>substations</dt>
				<dd>{lc.substations.points.length}</dd>
			</div>
		</dl>
		<p class="footnote mono">
			display only: positions inferred from the PowerWorld diagram, not surveyed latitude and
			longitude
		</p>
		<p class="footnote mono">decoded in your browser by powerio (wasm); never uploaded</p>
	{:else if lc.summary}
		<dl class="mono">
			<div>
				<dt>buses</dt>
				<dd>{lc.summary.n_bus}</dd>
			</div>
			<div>
				<dt>branches</dt>
				<dd>{lc.summary.n_branch}</dd>
			</div>
			<div>
				<dt>generators</dt>
				<dd>{lc.summary.n_gen}</dd>
			</div>
			<div>
				<dt>load</dt>
				<dd>{fmt.format(lc.summary.load_mw)} MW</dd>
			</div>
			<div>
				<dt>gen capacity</dt>
				<dd>{fmt.format(lc.summary.gen_mw)} MW</dd>
			</div>
			<div>
				<dt>base MVA</dt>
				<dd>{fmt.format(lc.summary.base_mva)}</dd>
			</div>
		</dl>
		{#if lc.summary.warnings.length > 0}
			<ul class="warnings mono">
				{#each lc.summary.warnings.slice(0, 4) as w, i (i)}
					<li>{w}</li>
				{/each}
				{#if lc.summary.warnings.length > 4}
					<li>+{lc.summary.warnings.length - 4} more</li>
				{/if}
			</ul>
		{/if}
		{#if !lc.view}
			<p class="footnote mono">
				no coordinates in this file: click the map or drop a geographic file
			</p>
		{:else if lc.coordsKind === 'synthetic' || lc.coordsKind === 'manual'}
			<p class="footnote mono">
				coordinates: {lc.coordsKind} topology layout centered where it was placed
			</p>
		{:else if lc.coordsKind === 'geofile'}
			<p class="footnote mono">
				coordinates: geographic file data from {lc.geoSource}
			</p>
		{/if}
		{#if lc.geoWarnings && lc.geoWarnings.length > 0}
			<ul class="warnings mono">
				{#each lc.geoWarnings.slice(0, 4) as w, i (i)}
					<li>{w}</li>
				{/each}
				{#if lc.geoWarnings.length > 4}
					<li>+{lc.geoWarnings.length - 4} more</li>
				{/if}
			</ul>
		{/if}
		<p class="footnote mono">parsed in your browser by powerio (wasm); never uploaded</p>
		{#if lc.networkJson}
			<div class="study">
				<button class="reset mono" disabled={busy} onclick={() => saveStudy(lc)}>
					save study (.pio.json)
				</button>
				<div class="export">
					<button
						class="reset mono"
						disabled={busy}
						aria-expanded={exportOpen}
						onclick={() => (exportOpen = !exportOpen)}
					>
						export committed state…
					</button>
					{#if exportOpen}
						<ul class="export-menu mono">
							{#each EXPORT_FORMATS as f (f.token)}
								<li>
									<button class="reset mono" disabled={busy} onclick={() => exportStudy(lc, f.token)}>
										{f.label}
									</button>
								</li>
							{/each}
						</ul>
					{/if}
				</div>
				{#if lc.view}
					<button class="reset mono" disabled={busy} onclick={() => downloadLayout(lc)}>
						download layout (.geo.json)
					</button>
				{/if}
			</div>
			{#if exportWarnings.length > 0}
				<ul class="warnings mono">
					{#each exportWarnings.slice(0, 4) as w, i (i)}
						<li>{w}</li>
					{/each}
					{#if exportWarnings.length > 4}
						<li>+{exportWarnings.length - 4} more</li>
					{/if}
				</ul>
			{/if}
		{/if}
	{/if}
	{#if lc.topology && lc.coordsKind !== 'file'}
		<button class="reset mono" onclick={() => ctrl.moveLocalCase(lc)}>
			{lc.coordsKind === 'synthetic_pending'
				? 'place on map'
				: lc.coordsKind === 'geofile'
					? 'place manually'
					: 'move layout'}
		</button>
	{/if}
	<button class="reset mono" onclick={() => ctrl.removeLocalCase(lc)}>remove</button>
{/if}

<style>
	h2 {
		margin: 0 0 12px;
		font-size: 16px;
		font-weight: 600;
	}

	dl {
		margin: 0;
		font-size: 12.5px;
	}

	dl div {
		display: flex;
		justify-content: space-between;
		padding: 3px 0;
	}

	dt {
		color: var(--text-secondary);
	}

	dd {
		margin: 0;
	}

	.footnote {
		margin: 8px 0 0;
		font-size: 10px;
		color: var(--text-tertiary);
		letter-spacing: 0;
	}

	.warnings {
		margin: 8px 0 0;
		padding: 0;
		list-style: none;
		font-size: 10.5px;
		line-height: 1.5;
		color: var(--text-accent);
	}

	.study {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		margin-top: 10px;
	}

	.export {
		position: relative;
	}

	.export-menu {
		position: absolute;
		z-index: 1;
		margin: 4px 0 0;
		padding: 4px;
		list-style: none;
		min-width: 100%;
		background: var(--surface, #fff);
		border: 1px solid var(--border, rgba(0, 0, 0, 0.15));
		border-radius: 6px;
		box-shadow: 0 6px 20px rgba(0, 0, 0, 0.18);
	}

	.export-menu li {
		white-space: nowrap;
	}
</style>
