<script lang="ts">
	import { getAppState, getController } from '$lib/context.svelte';
	import { fmt } from '$lib/format';

	const app = getAppState();
	const ctrl = getController();
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
			display only &mdash; positions inferred from the PowerWorld diagram, not surveyed latitude
			and longitude
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
				no coordinates in this file &mdash; click the map or drop a geographic file
			</p>
		{:else if lc.coordsKind === 'synthetic'}
			<p class="footnote mono">
				coordinates: synthetic topology layout centered where you placed it
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
		color: var(--ink-dim);
	}

	dd {
		margin: 0;
	}

	.footnote {
		margin: 8px 0 0;
		font-size: 10px;
		color: var(--ink-dim);
		letter-spacing: 0;
	}

	.warnings {
		margin: 8px 0 0;
		padding: 0;
		list-style: none;
		font-size: 10.5px;
		line-height: 1.5;
		color: var(--accent);
	}
</style>
