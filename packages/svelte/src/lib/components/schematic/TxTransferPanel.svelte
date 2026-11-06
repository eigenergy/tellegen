<script lang="ts">
	import type { NetworkBranch } from '@tellegen/engine';
	import { formulationLabel, readable } from '../../format.js';
	import type { operatingContext } from '../../diagram/transmission-results.js';
	import {
		TRANSFER_SCALES,
		transferDirectionText,
		type TransferReading
	} from '../../diagram/transmission-transfer.js';
	import { TERMS, transferLead } from '../../diagram/transmission-explain.js';
	import TxPanelHead from './TxPanelHead.svelte';
	import { previewHandlers, type PreviewItem } from './preview.js';

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
		onpreview?: (item: PreviewItem | null) => void;
		/** Collapsed to the heading and the width scale. */
		brief?: boolean;
	} = $props();

	let open = $state(false);
	const lead = $derived.by(() => {
		const first = transfers[0];
		return transferLead({
			hasResult: !!context.current,
			awaiting: context.solved && !context.current,
			missing,
			zero,
			top: first
				? {
						id: first.branch.id,
						mw: readable(first.reading.magnitude, 'MW').text,
						direction: transferDirectionText(first.reading, first.branch)
					}
				: null,
			scale
		});
	});
</script>

<section class="transfer" aria-label="Power transfers">
	<TxPanelHead title="Power transfers" {formulation} {context} {lead} view="transfer" {brief} />
	<p>
		<span class="blue">Blue line width</span> ·
		<span
			title="{TERMS.activePower} Each line shows the larger of its two ends' |P| (the ends differ by the losses)."
			>active power</span
		>
		· 0 to {scale} MW
	</p>
	{#if !brief}<label>
			Width scale
			<select value={scale} onchange={(event) => onscale(Number(event.currentTarget.value))}>
				{#each TRANSFER_SCALES as value (value)}<option {value}>{value} MW</option>{/each}
			</select>
			<span>Lines stop widening at the selected MW.</span>
		</label>
		<p>Chevrons show direction close in · Red gauges show overload in percent of rating.</p>
		<p>
			{zero} carrying no power · {missing} without a flow value
			{#if dc}
				· <span title={TERMS.lossless}>lossless (DC)</span>{/if}
		</p>
		<p>
			{hasDispatch
				? 'Bus readouts show solved generation and model demand.'
				: 'Solved generation unavailable. Bus readouts show model demand.'}
		</p>
		<details>
			<summary>MW, MVAr and MVA explained</summary>
			<p>{TERMS.activePower}</p>
			<p>{TERMS.lossless}</p>
		</details>
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
								{...previewHandlers(onpreview, {
									kind: 'branch',
									id: branch.id
								})}
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
	[title] {
		text-decoration: underline dotted;
		text-underline-offset: 2px;
		cursor: help;
	}
	.transfer {
		width: 100%;
		min-width: 0;
		font: 11px var(--font-mono);
	}
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
