<script lang="ts">
	import type { NetworkBranch } from '@tellegen/engine';
	import { formulationLabel, readable } from '../../format.js';
	import type { operatingContext } from '../../diagram/transmission-results.js';
	import {
		TRANSFER_SCALES,
		transferDirectionText,
		type TransferReading
	} from '../../diagram/transmission-transfer.js';

	let {
		formulation,
		context,
		dc,
		scale,
		onscale,
		transfers,
		zero,
		missing,
		hasDispatch,
		onbranch,
		onpreview,
		brief = false
	}: {
		formulation: Parameters<typeof formulationLabel>[0];
		context: ReturnType<typeof operatingContext>;
		dc: boolean;
		scale: number;
		onscale: (scale: number) => void;
		transfers: { branch: NetworkBranch; reading: TransferReading }[];
		zero: number;
		missing: number;
		hasDispatch: boolean;
		onbranch: (id: number) => void;
		onpreview?: (item: { kind: 'branch'; id: number } | null) => void;
		/** Collapsed to the heading and the width scale. */
		brief?: boolean;
	} = $props();

	let open = $state(false);
</script>

<section class="transfer" aria-label="Power transfers">
	<header>
		<strong>Power transfers</strong>
		<span>{formulationLabel(formulation)}</span>
		{#if context.relaxed}<strong>Relaxation</strong>{/if}
	</header>
	<p role="status" hidden={brief && !context.stale}>
		{context.freshness} · {context.status} · base case
	</p>
	<p><span class="blue">Blue line width</span> · Largest reported endpoint |P| · 0 to {scale} MW</p>
	{#if !brief}<label>
			Width scale
			<select value={scale} onchange={(event) => onscale(Number(event.currentTarget.value))}>
				{#each TRANSFER_SCALES as value (value)}<option {value}>{value} MW</option>{/each}
			</select>
			<span>Lines stop widening at the selected MW.</span>
		</label>
		<p>Chevrons show direction close in · Red gauges show overload in percent of rating.</p>
		<p>
			{zero} zero flow · {missing} active flow unavailable
			{#if dc}
				· Lossless DC endpoints{/if}
		</p>
		<p>
			{hasDispatch
				? 'Bus readouts show solved generation and model demand.'
				: 'Solved generation unavailable. Bus readouts show model demand.'}
		</p>
		<details bind:open>
			<summary>Largest active transfers</summary>
			{#if transfers.length}
				<ol>
					{#each transfers as { branch, reading } (branch.id)}
						<li>
							<button
								onclick={() => {
									open = false;
									onpreview?.(null);
									onbranch(branch.id);
								}}
								onpointerenter={() => onpreview?.({ kind: 'branch', id: branch.id })}
								onpointerleave={() => onpreview?.(null)}
								onfocus={() => onpreview?.({ kind: 'branch', id: branch.id })}
								onblur={() => onpreview?.(null)}
							>
								<strong>Branch {branch.id}</strong><span
									>{transferDirectionText(reading, branch)} · {readable(reading.magnitude, 'MW')
										.text}</span
								>
							</button>
						</li>
					{/each}
				</ol>
			{:else}<p>No nonzero transfers reported.</p>{/if}
		</details>{/if}
</section>

<style>
	.transfer {
		width: 100%;
		min-width: 0;
		font: 11px var(--font-mono);
	}
	header,
	label {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 4px 10px;
	}
	strong {
		color: var(--ink);
	}
	.blue {
		color: #1b6c97;
		font-weight: 600;
	}
	p {
		margin: 5px 0;
	}
	details {
		border-top: 1px solid var(--line);
		margin-top: 4px;
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
