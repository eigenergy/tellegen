<script lang="ts">
	import type { NetworkBranch, NetworkBus, SolutionFlow } from '@tellegen/engine';
	import { priceGradient } from '../../colors.js';
	import { displayFmt, formulationLabel, readable } from '../../format.js';
	import {
		PRICE_SCALES,
		PRICE_UNIT,
		economicAmount,
		isOpf,
		thermalStatus,
		type economicsFor
	} from '../../diagram/transmission-economics.js';
	import type { operatingContext } from '../../diagram/transmission-results.js';
	import { economicsLead } from '../../diagram/transmission-explain.js';
	import TxPanelHead from './TxPanelHead.svelte';
	import { previewHandlers, type PreviewItem } from './preview.js';

	let {
		formulation,
		context,
		economics,
		scale,
		onscale,
		domain,
		bus,
		branch,
		busPrice,
		flow,
		onbranch,
		onpreview,
		shadingCentre = null,
		brief = false
	}: {
		formulation: Parameters<typeof formulationLabel>[0];
		context: ReturnType<typeof operatingContext>;
		economics: ReturnType<typeof economicsFor>;
		/** The chosen scale: -1 fits it to the prices, as the map's legend does. */
		scale: number;
		onscale: (scale: number) => void;
		/** The colour scale's ends in $/MWh. */
		domain: { lo: number; hi: number };
		bus?: NetworkBus;
		branch?: NetworkBranch;
		busPrice?: number;
		flow?: SolutionFlow;
		onbranch: (id: number) => void;
		onpreview?: (item: PreviewItem | null) => void;
		/** The median LMP when the area shading shows price separation around it. */
		shadingCentre?: number | null;
		/** Collapsed to the heading, the price scale and the binding count. */
		brief?: boolean;
	} = $props();
	const preview = (id: number) => previewHandlers(onpreview, { kind: 'branch', id });
	const branchResult = $derived(branch ? economics.branches.get(branch.id) : undefined);
	let limit = $state(12);
	const lead = $derived(
		economicsLead({
			opf: isOpf(formulation),
			range: economics.range,
			uniform: economics.uniform
		})
	);
</script>

<section class="economics" aria-label="Prices">
	<TxPanelHead title="Prices" {context} {lead} view="economics" />
	{#if economics.uniform === null && economics.prices.length}
		<div class="scale">
			<span
				>{economics.range && economics.range.lo < domain.lo ? '≤' : ''}{displayFmt(
					'price',
					domain.lo
				)}</span
			><i style:background={priceGradient}></i><span
				>{economics.range && economics.range.hi > domain.hi ? '≥' : ''}{displayFmt(
					'price',
					domain.hi
				)}
				{PRICE_UNIT}</span
			>
		</div>
		{#if !brief}<label>
				Scale
				<select value={scale} onchange={(event) => onscale(Number(event.currentTarget.value))}>
					<option value={-1}>Fit to prices</option>
					{#each PRICE_SCALES as entry, index (index)}
						<option value={index}>{entry.lo} to {entry.hi}</option>
					{/each}
				</select>
			</label>{/if}
		{#if shadingCentre !== null && !brief}
			<ul class="key">
				<li><i class="swatch cheap"></i>Below median</li>
				<li><i class="swatch dear"></i>Above median</li>
			</ul>
		{/if}
	{/if}
	{#if !brief}
		{#if bus || branch}
			<dl class="pick">
				{#if bus}
					<div>
						<dt>Bus {bus.id}</dt>
						<dd>{economicAmount(busPrice)}</dd>
					</div>
				{/if}
				{#if branch}
					<div>
						<dt>Line {branch.id}</dt>
						<dd>{thermalStatus(branchResult)}</dd>
					</div>
					<div>
						<dt>Value of rating</dt>
						<dd>{economicAmount(branchResult?.relief_per_mw)}/MW</dd>
					</div>
				{/if}
			</dl>
		{/if}
		{#if economics.thermalAvailable}
			<ul class="key">
				<li><i class="swatch binding"></i>At limit</li>
			</ul>
			<details>
				<summary>Lines at limit ({economics.binding.length})</summary>
				{#if economics.binding.length}
					<ol>
						{#each economics.binding.slice(0, limit) as row (row.branch)}
							<li>
								<button
									aria-pressed={row.branch === branch?.id}
									onclick={() => onbranch(row.branch)}
									{...preview(row.branch)}
								>
									<strong>Line {row.branch}</strong>
									<span>{economicAmount(row.relief_per_mw)}/MW</span>
								</button>
							</li>
						{/each}
					</ol>
					{#if economics.binding.length > limit}<button onclick={() => (limit += 12)}
							>Show more</button
						>{/if}
				{:else}<p class="dim">None</p>{/if}
			</details>
		{/if}
		{#if economics.constraints.length}
			<details>
				<summary>Constraints ({economics.constraints.length})</summary>
				{#each economics.constraints as row (row.id)}
					<details>
						<summary>{row.id}{row.binding ? ', at limit' : ''}</summary>
						<dl class="pick">
							<div>
								<dt>Value</dt>
								<dd>{readable(row.value, 'MW').text}</dd>
							</div>
							<div>
								<dt>Min</dt>
								<dd>{row.lower ?? '—'}</dd>
							</div>
							<div>
								<dt>Max</dt>
								<dd>{row.upper ?? '—'}</dd>
							</div>
							<div>
								<dt>Shadow price</dt>
								<dd>{economicAmount(row.shadow_price)}</dd>
							</div>
						</dl>
						{#each row.branches ?? [] as id (id)}
							<button aria-pressed={id === branch?.id} onclick={() => onbranch(id)}
								>Line {id}</button
							>
						{/each}
					</details>
				{/each}
			</details>
		{/if}
	{/if}
</section>

<style>
	.economics {
		width: 100%;
		min-width: 0;
		font-family: var(--font-display);
		font-size: var(--fs-label);
		line-height: var(--lh-snug);
	}
	.scale,
	label {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: var(--space-2xs) var(--space-md);
	}
	.scale > span {
		font-family: var(--font-mono);
		font-size: var(--fs-control);
		font-variant-numeric: tabular-nums;
	}
	strong {
		color: var(--ink);
	}
	.key {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2xs) var(--space-lg);
		margin: var(--space-2xs) 0;
		padding: 0;
		list-style: none;
	}
	.key li {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.swatch {
		width: 18px;
		height: 8px;
		border-radius: 2px;
	}
	.swatch.cheap {
		background: #2166ac;
	}
	.swatch.dear {
		background: #9a5a2c;
	}
	.swatch.binding {
		height: 4px;
		background: var(--data-pos);
	}
	.pick {
		margin: var(--space-2xs) 0;
		font-variant-numeric: tabular-nums;
	}
	.pick div {
		display: flex;
		justify-content: space-between;
		gap: var(--space-sm);
		padding: 2px 0;
	}
	dt {
		color: var(--text-secondary);
	}
	dd {
		margin: 0;
		text-align: right;
	}
	.dim {
		color: var(--text-secondary);
	}
	p {
		margin: var(--space-2xs) 0;
	}
	i {
		width: 90px;
		height: 7px;
	}
	details {
		border-top: 1px solid var(--line);
		margin-top: var(--space-2xs);
	}
	details details {
		margin-left: var(--space-sm);
	}
	summary {
		padding: var(--space-xs) 0;
		cursor: pointer;
	}
	ol {
		margin: var(--space-2xs) 0;
		padding-left: var(--space-3xl);
	}
	li {
		padding: var(--space-3xs) 0;
	}
	li button {
		width: 100%;
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2xs) var(--space-lg);
		justify-content: space-between;
		text-align: left;
	}
	li button span {
		font-family: var(--font-mono);
		font-size: var(--fs-control);
		font-variant-numeric: tabular-nums;
		text-align: right;
	}
	button {
		padding: var(--space-2xs) var(--space-xs);
		font: inherit;
		color: var(--ink);
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: var(--radius-sm);
		cursor: pointer;
	}
	button[aria-pressed='true'] {
		background: var(--accent-soft);
	}
	@media (pointer: coarse) {
		button,
		select,
		summary {
			min-height: 44px;
		}
		summary {
			align-content: center;
		}
	}
</style>
