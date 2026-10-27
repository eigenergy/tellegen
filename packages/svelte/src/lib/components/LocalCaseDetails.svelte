<script lang="ts">
	import type { Snippet } from 'svelte';
	import { getAppState, getController } from '../context.svelte.js';
	import { fmt } from '../format.js';
	import type { LocalCase } from '../state.svelte.js';
	import { formatPowerIoDiagnostic, type PowerIoDiagnostic } from '@tellegen/engine';

	/** Optional host content rendered under the case stats. */
	let { note }: { note?: Snippet<[LocalCase]> } = $props();

	const app = getAppState();
	const ctrl = getController();

	// Grid exchange formats the committed state can export.
	const EXPORT_FORMATS = [
		{ token: 'matpower', label: 'MATPOWER (.m)' },
		{ token: 'psse', label: 'PSS/E (.raw)' },
		{ token: 'psse35', label: 'PSS/E v35 (.raw)' },
		{ token: 'powerworld', label: 'PowerWorld (.aux)' },
		{ token: 'pslf', label: 'PSLF (.epc)' },
		{ token: 'powermodels-json', label: 'PowerModels (.json)' },
		{ token: 'pandapower-json', label: 'pandapower (.json)' }
	];

	let busy = $state(false);
	let exportOpen = $state(false);
	let exportDiagnostics = $state<PowerIoDiagnostic[]>([]);

	/** Where the map coordinates came from. */
	function coordinateSource(lc: LocalCase): string {
		if (!lc.view) return 'None';
		switch (lc.coordsKind) {
			case 'file':
				return 'From file';
			case 'geofile':
				return lc.geoSource ? `From ${lc.geoSource}` : 'From geographic file';
			case 'manual':
				return 'Manual layout';
			case 'synthetic':
				return lc.summary?.coords_method === 'generated' ? 'Generated' : 'Synthetic layout';
			default:
				return 'None';
		}
	}

	async function saveCase(lc: LocalCase) {
		busy = true;
		exportDiagnostics = [];
		try {
			await ctrl.saveCaseModule(lc);
		} finally {
			busy = false;
		}
	}

	async function saveSolution(lc: LocalCase) {
		busy = true;
		exportDiagnostics = [];
		try {
			await ctrl.saveSolutionModule(lc);
		} finally {
			busy = false;
		}
	}

	async function exportCase(lc: LocalCase, token: string) {
		busy = true;
		try {
			exportDiagnostics = await ctrl.exportCaseAs(lc, token);
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
	<h2>{lc.label}</h2>
	{#if lc.substations}
		<dl>
			<div>
				<dt>Substations</dt>
				<dd class="mono">{lc.substations.points.length}</dd>
			</div>
			<div>
				<dt>Positions</dt>
				<dd>Inferred from diagram</dd>
			</div>
			<div>
				<dt>File</dt>
				<dd class="mono">{lc.fileName}</dd>
			</div>
		</dl>
	{:else if lc.summary}
		<dl>
			<div>
				<dt>Buses</dt>
				<dd class="mono">{lc.summary.n_bus}</dd>
			</div>
			<div>
				<dt>Branches</dt>
				<dd class="mono">{lc.summary.n_branch}</dd>
			</div>
			<div>
				<dt>Generators</dt>
				<dd class="mono">{lc.summary.n_gen}</dd>
			</div>
			<div>
				<dt>Load</dt>
				<dd class="mono">{fmt.format(lc.summary.load_mw)} MW</dd>
			</div>
			<div>
				<dt>Generation capacity</dt>
				<dd class="mono">{fmt.format(lc.summary.gen_mw)} MW</dd>
			</div>
			<div>
				<dt>Base MVA</dt>
				<dd class="mono">{fmt.format(lc.summary.base_mva)}</dd>
			</div>
			{#if lc.displayMode === 'diagram' && lc.diagram}
				<div>
					<dt>Drawing</dt>
					<dd>{lc.diagram.name}</dd>
				</div>
			{:else}
				<div>
					<dt>Coordinates</dt>
					<dd>{coordinateSource(lc)}</dd>
				</div>
			{/if}
			<div>
				<dt>File</dt>
				<dd class="mono">{lc.fileName}</dd>
			</div>
		</dl>
		{#if lc.displayMode === 'diagram' && lc.diagram?.warnings.length}
			<details>
				<summary>Drawing details</summary>
				<ul class="warnings">
					{#each lc.diagram.warnings as warning, i (i)}<li>{warning}</li>{/each}
				</ul>
			</details>
		{/if}
	{/if}
	{@render note?.(lc)}
	{#if lc.summary && lc.studyInputJson}
		{@const exportUnavailable = ctrl.caseExportUnavailableReason(lc)}
		<div class="case-actions">
			<button
				class="reset"
				disabled={busy || !!exportUnavailable}
				title={exportUnavailable ?? undefined}
				onclick={() => saveCase(lc)}>Save case</button
			>
			{#if lc.formulation === 'dcopf' && lc.solution}
				<button class="reset" disabled={busy} onclick={() => saveSolution(lc)}>Save solution</button
				>
			{/if}
			<button
				class="reset"
				disabled={busy || !!exportUnavailable}
				title={exportUnavailable ?? undefined}
				aria-expanded={exportOpen}
				aria-controls="case-export-formats"
				onclick={() => (exportOpen = !exportOpen)}
			>
				Export as
			</button>
			{#if lc.view || lc.diagram}
				<button class="reset" disabled={busy} onclick={() => downloadLayout(lc)}>
					{lc.displayMode === 'diagram' ? 'Save drawing' : 'Save geography'}
				</button>
			{/if}
		</div>
		{#if exportUnavailable}
			<p class="unavailable">{exportUnavailable}</p>
		{:else if exportOpen}
			<ul class="case-actions" id="case-export-formats" aria-label="Export formats">
				{#each EXPORT_FORMATS as f (f.token)}
					<li>
						<button class="reset" disabled={busy} onclick={() => exportCase(lc, f.token)}>
							{f.label}
						</button>
					</li>
				{/each}
			</ul>
		{/if}
		{#if exportDiagnostics.length > 0}
			<ul class="warnings">
				{#each exportDiagnostics.slice(0, 4) as diagnostic, i (i)}
					<li>{formatPowerIoDiagnostic(diagnostic)}</li>
				{/each}
				{#if exportDiagnostics.length > 4}
					<li>+{exportDiagnostics.length - 4} more</li>
				{/if}
			</ul>
		{/if}
	{/if}
	{#if lc.displayMode !== 'diagram' && lc.topology && lc.coordsKind !== 'file'}
		<button class="reset" onclick={() => ctrl.moveLocalCase(lc)}>
			{lc.coordsKind === 'synthetic_pending'
				? 'Place on map'
				: lc.coordsKind === 'geofile'
					? 'Place manually'
					: 'Move layout'}
		</button>
	{/if}
	<button class="reset" onclick={() => ctrl.removeLocalCase(lc)}>Remove</button>
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
		gap: 12px;
		padding: 3px 0;
	}

	dt {
		flex: none;
		color: var(--text-secondary);
	}

	dd {
		min-width: 0;
		margin: 0;
		text-align: right;
		overflow-wrap: anywhere;
	}

	details {
		margin-top: 8px;
		font-size: 12px;
	}

	.warnings {
		margin: 8px 0 0;
		padding: 0;
		list-style: none;
		font-size: 12.5px;
		line-height: 1.5;
		color: var(--text-accent);
	}

	.case-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		margin: 10px 0 0;
		padding: 0;
		list-style: none;
	}

	.case-actions .reset {
		margin-top: 0;
	}

	.unavailable {
		margin: 8px 0 0;
		font-size: 12px;
		color: var(--text-secondary);
	}
</style>
