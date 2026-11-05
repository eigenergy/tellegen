<script lang="ts">
	import type { NetworkBranch, NetworkBus, SolutionFlow } from '@tellegen/engine';
	import { priceGradient } from '../../colors.js';
	import { formulationLabel } from '../../format.js';
	import {
		PRICE_SCALES,
		PRICE_UNIT,
		economicAmount,
		thermalStatus,
		type economicsFor
	} from '../../diagram/transmission-economics.js';
	import type { operatingContext } from '../../diagram/transmission-results.js';

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
		onbranch
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
	} = $props();
	const domain = $derived(PRICE_SCALES[scale]);
	const branchResult = $derived(branch ? economics.branches.get(branch.id) : undefined);
	let limit = $state(12);
</script>

<section class="economics" aria-label="OPF economics">
	<header>
		<strong>OPF economics</strong>
		<span>{formulationLabel(formulation)}</span>
		{#if context.relaxed}<strong class="accent">Relaxation</strong>{/if}
	</header>
	<p role="status">{context.freshness} · {context.status} · base case</p>
	{#if economics.uniform !== null}
		<p>Total LMP uniform at {economicAmount(economics.uniform)}</p>
	{:else if economics.prices.length}
		<div class="scale">
			<span>Total LMP</span><span>{domain.lo}</span><i style:background={priceGradient}></i>
			<span>{domain.hi} {PRICE_UNIT}</span>
		</div>
		<label>
			Price scale
			<select value={scale} onchange={(event) => onscale(Number(event.currentTarget.value))}>
				{#each PRICE_SCALES as entry, index (index)}
					<option value={index}>{entry.lo} to {entry.hi}</option>
				{/each}
			</select>
			<span>Values outside the scale use its end colours.</span>
		</label>
	{:else}
		<p>Total LMP unavailable for this result.</p>
	{/if}
	{#if bus}
		<p><strong>Bus {bus.id}</strong> · Total LMP {economicAmount(busPrice)}</p>
	{/if}
	{#if branch}
		<p><strong>Branch {branch.id}</strong> · {thermalStatus(branchResult)}</p>
		<p>
			Marginal rating relief {economicAmount(branchResult?.relief_per_mw)}
			{#if flow?.loading_info?.rating_source === 'synthesized'}
				· Synthesized solver bound{/if}
		</p>
	{/if}
	{#if economics.thermalAvailable}
		<p>
			<span class="accent">Purple branch halo</span> · Binding thermal limit with positive marginal relief
		</p>
		<details>
			<summary>Binding thermal limits · {economics.binding.length}</summary>
			{#if economics.binding.length}
				<ol>
					{#each economics.binding.slice(0, limit) as row (row.branch)}
						<li>
							<button aria-pressed={row.branch === branch?.id} onclick={() => onbranch(row.branch)}>
								<strong>Branch {row.branch}</strong>
								<span>{economicAmount(row.relief_per_mw)} · marginal relief</span>
							</button>
						</li>
					{/each}
				</ol>
				{#if economics.binding.length > limit}<button onclick={() => (limit += 12)}
						>Show more thermal limits</button
					>{/if}
			{:else}<p>No binding thermal limits in this result.</p>{/if}
		</details>
	{:else}
		<p>Thermal enforcement and marginal relief unavailable for this result.</p>
	{/if}
	{#if economics.constraints.length}
		<details>
			<summary>Interface and linear constraints · {economics.constraints.length}</summary>
			{#each economics.constraints as row (row.id)}
				<details>
					<summary
						>{row.id} · {row.binding ? 'Binding' : 'Slack'} · {economicAmount(
							row.shadow_price
						)}</summary
					>
					<p>
						Enforced · {row.value.toFixed(6)} MW · lower {row.lower ?? 'none'} · upper {row.upper ??
							'none'}
					</p>
					<p>
						Signed shadow price {economicAmount(row.shadow_price)} · objective reduction for an increase
						in the signed limit
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
</section>

<style>
	.economics {
		width: 100%;
		min-width: 0;
		font: 11px var(--font-mono);
	}
	header,
	.scale,
	label {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 4px 10px;
	}
	strong {
		color: var(--ink);
	}
	.accent {
		color: #762a83;
	}
	p {
		margin: 5px 0;
	}
	i {
		width: 90px;
		height: 7px;
	}
	details {
		border-top: 1px solid var(--line);
		margin-top: 4px;
	}
	details details {
		margin-left: 8px;
	}
	summary {
		padding: 6px 0;
		cursor: pointer;
	}
	ol {
		margin: 4px 0;
		padding-left: 24px;
	}
	li {
		padding: 2px 0;
	}
	li button {
		width: 100%;
		display: flex;
		flex-wrap: wrap;
		gap: 4px 12px;
		justify-content: space-between;
		text-align: left;
	}
	button,
	select {
		padding: 4px 6px;
		font: inherit;
		color: var(--ink);
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: 3px;
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
