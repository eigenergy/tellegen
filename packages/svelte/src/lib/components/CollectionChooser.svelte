<script lang="ts">
	import { getController } from '../context.svelte.js';
	import type { CollectionRow } from '../collection-import.svelte.js';
	const model = getController().collectionImport;
	let dialog: HTMLDialogElement;
	let query = $state('');
	let page = $state(0);
	let explicitSession = $state('');
	let explicitPath = $state('');
	let explicitFormat = $state('dss');
	let exportScope = $state<'selected' | 'project'>('selected');
	const matching = $derived(
		model.rows.filter((row) => row.label.toLowerCase().includes(query.toLowerCase()))
	);
	const visible = $derived(matching.slice(page * 100, (page + 1) * 100));
	const selected = $derived(model.rows.filter((row) => row.checked));
	const inventory = $derived(
		model.collections.find((c) => String(c.id) === explicitSession)?.inspection.entries ?? []
	);
	const done = (row: CollectionRow) =>
		['imported', 'view-only', 'incomplete'].includes(row.outcome);
	const importable = $derived(selected.filter((row) => !done(row)));
	$effect(() => {
		if (model.open && dialog && !dialog.open) {
			query = '';
			page = 0;
			dialog.showModal();
		}
		if (!model.open && dialog?.open) dialog.close();
	});
	async function download(id: number, row?: CollectionRow) {
		try {
			const bytes = await model.download(id, row, exportScope);
			const url = URL.createObjectURL(
				new Blob([bytes as Uint8Array<ArrayBuffer>], { type: 'application/zip' })
			);
			const link = document.createElement('a');
			link.href = url;
			link.download = row ? `case-${exportScope}.zip` : 'original-collection.zip';
			link.click();
			setTimeout(() => URL.revokeObjectURL(url), 1000);
		} catch (error) {
			if (error instanceof DOMException && error.name === 'AbortError') return;
			model.errors.push(error instanceof Error ? error.message : String(error));
		}
	}
</script>

<dialog
	bind:this={dialog}
	aria-labelledby="collection-title"
	oncancel={(event) => {
		event.preventDefault();
		model.close();
	}}
>
	<div class="heading">
		<h2 id="collection-title">Choose cases from collection</h2>
		<button onclick={() => model.close()}>{model.busy ? 'Cancel import' : 'Close'}</button>
	</div>
	<p class="progress" role="status">{model.progress}</p>
	<p>
		Each selection becomes an independent case. Importing does not run a calculation. Unsupported
		snapshots remain view-only with an explanation.
	</p>
	{#each model.errors as error, index (index)}<p class="error" role="alert">{error}</p>{/each}
	{#if model.collections.length}
		<details>
			<summary>Source collections and downloads</summary>
			{#each model.collections as collection (collection.id)}
				<p>
					{collection.label}: {collection.inspection.entries.length} files
					{#if collection.inspection.original_zip}<button
							disabled={model.busy}
							onclick={() => download(collection.id)}
							>Download original ZIP — {collection.label}</button
						>{/if}
				</p>
			{/each}
			<p>
				Original ZIP downloads preserve the uploaded bytes. Source exports create a new ZIP.
				“Project” includes all files within the selected scope, possibly other cases.
			</p>
			<label
				>Source export scope <select bind:value={exportScope}
					><option value="selected">Selected members only</option><option value="project"
						>Whole permitted project</option
					></select
				></label
			>
			<button
				disabled={model.busy || selected.length !== 1}
				onclick={() => download(selected[0].session, selected[0])}
				>Export selected source ZIP</button
			>
		</details>
	{/if}
	<div class="toolbar">
		<label
			>Filter candidates <input
				bind:value={query}
				oninput={() => {
					page = 0;
				}}
			/></label
		>
		<button
			disabled={model.busy}
			onclick={() =>
				matching.forEach((row) => {
					if (!done(row)) row.checked = true;
				})}>Select all matching</button
		>
		<button
			disabled={model.busy}
			onclick={() =>
				model.rows.forEach((row) => {
					row.checked = false;
				})}>Clear selection</button
		>
	</div>
	<div class="candidates">
		{#each visible as row (row.id)}
			<div class="candidate" data-outcome={row.outcome}>
				<label class="name"
					><input type="checkbox" bind:checked={row.checked} disabled={model.busy} />
					<span>{row.label}</span></label
				>
				<label class="format"
					>Format <input
						list="collection-formats"
						bind:value={row.format}
						disabled={model.busy || done(row) || row.selection.operation === 'deserialize'}
						placeholder={row.selection.operation === 'deserialize'
							? 'PowerIO IR'
							: 'Choose format if needed'}
						aria-label="Format for {row.label}"
					/></label
				>
				<span class="status"
					>{row.outcome === 'pending' ? row.status.replaceAll('-', ' ') : row.outcome}</span
				>
				{#if row.message}<p class:failure={row.outcome === 'failed'}>{row.message}</p>{/if}
			</div>
		{/each}
		{#if !visible.length && !model.busy}<p>
				No candidates match. Use the inventory below to select a nonstandard entry explicitly.
			</p>{/if}
	</div>
	{#if matching.length > 100}<div class="toolbar">
			<button disabled={page === 0} onclick={() => page--}>Previous candidates</button><span
				>Page {page + 1} of {Math.ceil(matching.length / 100)}</span
			><button disabled={(page + 1) * 100 >= matching.length} onclick={() => page++}
				>Next candidates</button
			>
		</div>{/if}
	<datalist id="collection-formats"
		>{#each ['dss', 'matpower', 'psse', 'psse-rawx', 'aux', 'pslf', 'pwb', 'pmd-json', 'bmopf-json', 'xiidm', 'jiidm', 'cgmes', 'goc3', 'pypsa-csv', 'gridlabd'] as format (format)}<option
				value={format}
			></option>{/each}</datalist
	>
	{#if model.collections.length}
		<details>
			<summary>Choose a nonstandard entry or document group</summary>
			<p>
				ZIP entries and XML documents may need an explicit format. Nested transport ZIPs are not
				expanded automatically. Group only documents belonging to the same case.
			</p>
			<label
				>Collection <select
					bind:value={explicitSession}
					onchange={() => {
						explicitPath = '';
					}}
					><option value="">Choose a collection</option
					>{#each model.collections as collection (collection.id)}<option value={collection.id}
							>{collection.label}</option
						>{/each}</select
				></label
			>
			<label
				>Entry <select bind:value={explicitPath}
					><option value="">Choose an entry</option>{#each inventory as entry (entry.path)}<option
							value={entry.path}>{entry.path} ({entry.bytes} bytes)</option
						>{/each}</select
				></label
			>
			<label>Explicit format <input list="collection-formats" bind:value={explicitFormat} /></label>
			<button
				disabled={model.busy || !explicitPath || !explicitFormat}
				onclick={() => model.addExplicit(Number(explicitSession), explicitPath, explicitFormat)}
				>Add candidate</button
			>
			<div class="toolbar">
				<button disabled={model.busy} onclick={() => model.groupSelected('cgmes')}
					>Group selected as CGMES</button
				><button disabled={model.busy} onclick={() => model.groupSelected('goc3')}
					>Group selected as GO problem/solution</button
				>
			</div>
		</details>
	{/if}
	<p class="hint">
		Companion files remain available to their selected master. Separate geographic sidecars are not
		applied to every case: import first, then select a case and attach its sidecar.
	</p>
	<div class="footer">
		<span>{selected.length} selected; {importable.length} to import (up to 32 per import)</span
		><button
			class="primary"
			disabled={model.busy || !importable.length || importable.length > 32}
			onclick={() => model.importSelected()}>Import selected cases</button
		>
	</div>
</dialog>

<style>
	dialog {
		color: var(--ink);
		background: var(--bg);
		border: 1px solid var(--border);
		border-radius: 6px;
		width: min(1000px, calc(100vw - 40px));
		max-height: calc(100vh - 48px);
		padding: 20px;
		font-size: 13px;
	}
	dialog::backdrop {
		background: #0008;
	}
	.heading,
	.toolbar,
	.footer {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 10px;
	}
	.heading,
	.footer {
		justify-content: space-between;
	}
	h2 {
		font-size: 18px;
		margin: 0;
	}
	p {
		line-height: 1.5;
	}
	.progress {
		font-weight: 600;
	}
	label {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}
	button,
	input,
	select {
		font: inherit;
		color: inherit;
		background: var(--surface-chip);
		border: 1px solid var(--border);
		border-radius: 3px;
		padding: 6px 8px;
	}
	button {
		cursor: pointer;
	}
	button:disabled {
		opacity: 0.5;
		cursor: default;
	}
	.candidates {
		margin: 12px 0;
		max-height: 42vh;
		overflow: auto;
	}
	.candidate {
		display: grid;
		grid-template-columns: minmax(200px, 1fr) 220px 90px;
		gap: 8px;
		align-items: center;
		border-bottom: 1px solid var(--border);
		padding: 10px 2px;
	}
	.name span {
		overflow-wrap: anywhere;
	}
	.candidate p {
		grid-column: 1 / -1;
		margin: 0;
	}
	.format input {
		min-width: 0;
		width: 100%;
	}
	.status {
		font-size: 11px;
	}
	.error,
	.failure {
		color: var(--error, #b33);
	}
	.hint {
		font-size: 12px;
		color: var(--text-secondary);
	}
	details {
		margin: 10px 0;
	}
	summary {
		cursor: pointer;
	}
	.footer {
		margin-top: 16px;
	}
	.primary {
		border-color: var(--accent);
		font-weight: 600;
	}
	@media (max-width: 650px) {
		.candidate {
			grid-template-columns: 1fr;
		}
		.candidate p {
			grid-column: 1;
		}
	}
</style>
