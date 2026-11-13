<script lang="ts">
	import type { NetworkBranch, NetworkBus, SolutionFlow } from '@tellegen/engine';
	import { priceGradient } from '../../colors.js';
	import { formulationLabel, readable } from '../../format.js';
	import {
		PRICE_SCALES,
		PRICE_UNIT,
		economicAmount,
		isOpf,
		thermalStatus,
		type economicsFor
	} from '../../diagram/transmission-economics.js';
	import type { operatingContext } from '../../diagram/transmission-results.js';
	import { TERMS, economicsLead } from '../../diagram/transmission-explain.js';
	import TxPanelHead from './TxPanelHead.svelte';
	import { previewHandlers, type PreviewItem } from './preview.js';

	let {
		formulation,
		context,
		economics,
		scale,
		onscale,
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
		scale: number;
		onscale: (scale: number) => void;
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
	const domain = $derived(PRICE_SCALES[scale]);
	const branchResult = $derived(branch ? economics.branches.get(branch.id) : undefined);
	let limit = $state(12);
	const lead = $derived(
		economicsLead({
			opf: isOpf(formulation),
			ac: formulation === 'acopf' || formulation === 'socwr',
			range: economics.range,
			uniform: economics.uniform,
			binding: economics.binding.length,
			thermalAvailable: economics.thermalAvailable
		})
	);
</script>

<section class="economics" aria-label="OPF economics">
	<TxPanelHead title="OPF economics" {formulation} {context} {lead} view="economics" {brief} />
	{#if economics.uniform !== null}
		<p>
			<abbr title={TERMS.lmp}>Total LMP</abbr> is uniform at
			<span title={TERMS.objectiveUnits}>{economicAmount(economics.uniform)}</span>.
		</p>
	{:else if economics.prices.length}
		<div class="scale">
			<abbr title={TERMS.lmp}>Total LMP</abbr><span>{domain.lo}</span><i
				style:background={priceGradient}
			></i>
			<span
				>{domain.hi}
				<span title={TERMS.objectiveUnits}>{PRICE_UNIT}</span></span
			>
		</div>
		{#if !brief}<label>
				Price scale
				<select value={scale} onchange={(event) => onscale(Number(event.currentTarget.value))}>
					{#each PRICE_SCALES as entry, index (index)}
						<option value={index}>{entry.lo} to {entry.hi}</option>
					{/each}
				</select>
				<span>Values outside the scale use its end colours.</span>
			</label>{/if}
		{#if shadingCentre !== null && !brief}
			<p>
				Interpolated shading: <span class="cheap">cheaper</span> or
				<span class="dear">dearer</span>
				than the median LMP of {economicAmount(shadingCentre)}.
			</p>
		{/if}
	{:else if !isOpf(formulation)}
		<!-- The lead says where prices come from. -->
	{:else}
		<p>Total LMP unavailable for this result.</p>
	{/if}
	{#if brief}
		{#if economics.thermalAvailable}<p>
				Binding thermal limits ({economics.binding.length})
			</p>{/if}
	{:else}
		<details class="concepts">
			<summary>Loading, binding and marginal relief explained</summary>
			<p>
				<strong>At rating:</strong>
				{TERMS.atRating} This is a flow measurement, not a statement about solver enforcement.
			</p>
			<p><strong>Binding:</strong> {TERMS.binding}</p>
			<p><strong>Marginal relief:</strong> {TERMS.relief}</p>
			<p><strong>LMP:</strong> {TERMS.lmp} {TERMS.objectiveUnits}</p>
		</details>
		{#if bus}
			<p>
				<strong>Bus {bus.id}</strong>: total LMP {economicAmount(busPrice)}
			</p>
		{/if}
		{#if branch}
			<p><strong>Branch {branch.id}</strong>: {thermalStatus(branchResult)}</p>
			<p>
				<span title={TERMS.relief}>Marginal rating relief</span>
				{economicAmount(branchResult?.relief_per_mw)}
				{#if flow?.loading_info?.rating_source === 'synthesized'}
					(synthesized solver bound){/if}
			</p>
		{/if}
		{#if economics.thermalAvailable}
			<p>
				A <span class="accent">wide purple line</span> marks a
				<span title={TERMS.binding}>binding</span> line limit with positive
				<span title={TERMS.relief}>marginal relief</span>: increasing its rating can lower the
				optimal cost.
			</p>
			<details>
				<summary>Binding thermal limits ({economics.binding.length})</summary>
				{#if economics.binding.length}
					<ol>
						{#each economics.binding.slice(0, limit) as row (row.branch)}
							<li>
								<button
									aria-pressed={row.branch === branch?.id}
									onclick={() => onbranch(row.branch)}
									{...preview(row.branch)}
								>
									<strong>Branch {row.branch}</strong>
									<span title={TERMS.relief}
										>{economicAmount(row.relief_per_mw)} marginal relief</span
									>
								</button>
							</li>
						{/each}
					</ol>
					{#if economics.binding.length > limit}<button onclick={() => (limit += 12)}
							>Show more thermal limits</button
						>{/if}
				{:else}<p>No binding thermal limits in this result.</p>{/if}
			</details>
		{:else if isOpf(formulation)}
			<p>Thermal enforcement and marginal relief unavailable for this result.</p>
		{/if}
		{#if economics.constraints.length}
			<details>
				<summary>Interface and linear constraints ({economics.constraints.length})</summary>
				{#each economics.constraints as row (row.id)}
					<details>
						<summary
							>{row.id}: {row.binding ? 'binding' : 'not binding'}, {economicAmount(
								row.shadow_price
							)}</summary
						>
						<p>
							Enforced at {readable(row.value, 'MW').text}, lower bound {row.lower ?? 'none'}, upper
							bound {row.upper ?? 'none'}.
						</p>
						<p>
							Signed shadow price {economicAmount(row.shadow_price)}: the objective reduction for an
							increase in the signed limit.
						</p>
						{#each row.branches ?? [] as id (id)}
							<button aria-pressed={id === branch?.id} onclick={() => onbranch(id)}
								>Frame branch {id}</button
							>
						{/each}
					</details>
				{/each}
			</details>
		{/if}
	{/if}
</section>

<style>
	abbr,
	[title] {
		text-decoration: underline dotted;
		text-underline-offset: 2px;
		cursor: help;
	}
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
	.accent {
		color: var(--data-pos);
	}
	.cheap {
		color: #2166ac;
		font-weight: 600;
	}
	.dear {
		color: #9a5a2c;
		font-weight: 600;
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
	button,
	select {
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
