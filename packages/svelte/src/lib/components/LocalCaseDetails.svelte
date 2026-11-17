<script lang="ts">
	import { CASE_EXPORT_FORMATS } from '../export-formats.js';
	import type { Snippet } from 'svelte';
	import { getAppState, getController } from '../context.svelte.js';
	import { fmt } from '../format.js';
	import type { LocalCase } from '../state.svelte.js';
	import type { PowerIoDiagnostic } from '@tellegen/engine';
	import { getPanelLayout } from '../panels.svelte.js';
	import OutcomeVerdict from './OutcomeVerdict.svelte';
	import SynchronousAreas from './SynchronousAreas.svelte';

	/** Optional host content rendered under the case stats; `lead` goes right after
	 * the result sentence, such as a selection readout. */
	let { note, lead }: { note?: Snippet<[LocalCase]>; lead?: Snippet } = $props();

	const app = getAppState();
	const ctrl = getController();
	const layout = getPanelLayout();

	const EXPORT_FORMATS = CASE_EXPORT_FORMATS;

	let busy = $state(false);
	/** The File menu: the case's save, export and layout actions in one place. */
	let fileOpen = $state(false);
	let fileAnchor = $state<HTMLElement | undefined>();
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

	/** Run a File menu action and close the menu. */
	function pick(action: () => unknown) {
		fileOpen = false;
		exportOpen = false;
		void action();
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

<svelte:window
	onpointerdown={(event) => {
		if (fileOpen && !fileAnchor?.contains(event.target as Node)) fileOpen = false;
	}}
	onkeydown={(event) => {
		if (fileOpen && event.key === 'Escape') {
			event.preventDefault();
			fileOpen = false;
		}
	}}
/>

{#if app.activeLocal}
	{@const lc = app.activeLocal}
	<h2>{lc.label}</h2>
	<OutcomeVerdict />
	{@render lead?.()}
	{#if lc.substations}
		<dl>
			<div>
				<dt>Substations</dt>
				<dd class="mono">{lc.substations.points.length}</dd>
			</div>
			<div>
				<dt>Positions</dt>
				<dd>From diagram</dd>
			</div>
			<div>
				<dt>File</dt>
				<dd class="mono">{lc.fileName}</dd>
			</div>
		</dl>
	{:else if lc.summary}
		<!-- The same two rows as a hosted case; the rest of the file's data folds away. -->
		<dl>
			<div>
				<dt>Buses</dt>
				<dd class="mono">{lc.summary.n_bus.toLocaleString('en-US')}</dd>
			</div>
			<div>
				<dt>Lines</dt>
				<dd class="mono">{lc.summary.n_branch.toLocaleString('en-US')}</dd>
			</div>
		</dl>
		<details class="case-data">
			<summary>Case details</summary>
			<dl>
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
		</details>
		{#if lc.displayMode === 'diagram' && lc.diagram?.warnings.length}
			<details>
				<summary>Drawing details</summary>
				<ul class="warnings">
					{#each lc.diagram.warnings as warning, i (i)}<li>{warning}</li>{/each}
				</ul>
			</details>
		{/if}
	{/if}
	<SynchronousAreas
		moduleJson={lc.studyInputJson}
		selectedBus={app.selectedBus}
		onselect={(bus) => { void ctrl.selectLocalBus(lc.id, bus); }}
	/>
	{@render note?.(lc)}
	{@const exportUnavailable =
		lc.summary && lc.studyInputJson ? ctrl.caseExportUnavailableReason(lc) : 'Not available'}
	{@const canSave = !!(lc.summary && lc.studyInputJson)}
	{@const placeLabel =
		lc.displayMode !== 'diagram' && lc.topology && lc.coordsKind !== 'file'
			? lc.coordsKind === 'synthetic_pending'
				? 'Place on map'
				: lc.coordsKind === 'geofile'
					? 'Place manually'
					: 'Move layout'
			: null}
	<span class="file-menu" bind:this={fileAnchor}>
		<button
			class="reset"
			aria-expanded={fileOpen}
			aria-controls="case-file-menu"
			onclick={() => {
				fileOpen = !fileOpen;
				exportOpen = false;
			}}
			>File<svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"
				><path d="m3 4.5 3 3 3-3" fill="none" stroke="currentColor" stroke-width="1.5" /></svg
			></button
		>
		<div class="file-list" id="case-file-menu" role="group" aria-label="File" hidden={!fileOpen}>
			{#if canSave}
				<button
					disabled={busy || !!exportUnavailable}
					title={exportUnavailable ?? undefined}
					onclick={() => pick(() => saveCase(lc))}>Save case</button
				>
				{#if lc.formulation === 'dcopf' && lc.solution}
					<button disabled={busy} onclick={() => pick(() => saveSolution(lc))}>Save solution</button
					>
				{/if}
				<button
					disabled={busy || !!exportUnavailable}
					title={exportUnavailable ?? undefined}
					aria-expanded={exportOpen}
					aria-controls="case-export-formats"
					onclick={() => (exportOpen = !exportOpen)}>Export as</button
				>
				{#if exportOpen && !exportUnavailable}
					<ul id="case-export-formats" aria-label="Export formats">
						{#each EXPORT_FORMATS as f (f.token)}
							<li>
								<button disabled={busy} onclick={() => pick(() => exportCase(lc, f.token))}
									>{f.label}</button
								>
							</li>
						{/each}
					</ul>
				{/if}
				{#if lc.view || lc.diagram}
					<button disabled={busy} onclick={() => pick(() => downloadLayout(lc))}>
						{lc.displayMode === 'diagram' ? 'Save drawing' : 'Save geography'}
					</button>
				{/if}
				<span class="sep" aria-hidden="true"></span>
			{/if}
			{#if placeLabel}
				<button onclick={() => pick(() => ctrl.moveLocalCase(lc))}>{placeLabel}</button>
			{/if}
			<button onclick={() => pick(() => ctrl.removeLocalCase(lc))}>Remove</button>
		</div>
	</span>
	{#if exportDiagnostics.length > 0}
		<button class="link" onclick={() => layout.show('log')}
			>Export notes in Log ({exportDiagnostics.length})</button
		>
	{/if}
{/if}

<style>
	h2 {
		margin: 0 0 var(--space-lg);
		font-size: var(--fs-heading);
		font-weight: 600;
	}

	dl {
		margin: 0;
		font-size: var(--fs-data);
	}

	dl div {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		gap: var(--space-lg);
		padding: var(--space-3xs) 0;
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
		margin-top: var(--space-sm);
		font-size: var(--fs-label);
	}

	.case-data {
		margin-bottom: var(--space-md);
	}

	.case-data summary {
		color: var(--text-secondary);
		cursor: pointer;
	}

	.case-data dl {
		margin-top: var(--space-xs);
	}

	.warnings {
		margin: var(--space-sm) 0 0;
		padding: 0;
		list-style: none;
		font-size: var(--fs-data);
		line-height: var(--lh-normal);
		color: var(--text-secondary);
	}

	/* The File list opens in the panel's flow, so the panel's scrolling never clips it. */
	.file-menu {
		display: contents;
	}

	.file-menu > .reset {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		margin-right: var(--space-sm);
	}

	.file-list[hidden] {
		display: none;
	}

	.file-list {
		display: grid;
		max-width: 240px;
		margin: var(--space-xs) 0 var(--space-sm);
		padding: 4px;
		background: var(--raised);
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
	}

	.file-list ul {
		margin: 0;
		padding: 0 0 0 12px;
		list-style: none;
	}

	.file-list button {
		display: block;
		width: 100%;
		padding: 6px 8px;
		border: 0;
		border-radius: var(--radius-control);
		background: none;
		color: var(--ink);
		font: inherit;
		font-size: var(--fs-label);
		text-align: left;
		cursor: pointer;
	}

	.file-list button:hover:not(:disabled) {
		background: var(--surface-hover);
	}

	.file-list button:disabled {
		color: var(--text-tertiary);
		cursor: default;
	}

	.file-list .sep {
		margin: 4px 0;
		border-top: 1px solid var(--line);
	}

	.link {
		display: block;
		margin-top: var(--space-sm);
		padding: 0;
		border: 0;
		background: none;
		color: var(--text-accent);
		font: inherit;
		font-size: var(--fs-label);
		cursor: pointer;
	}

	@media (pointer: coarse) {
		.file-list button {
			min-height: 44px;
		}
	}
</style>
