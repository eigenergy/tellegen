<script lang="ts">
	import type { NetworkBranch } from '@tellegen/engine';
	import { formulationLabel, readable } from '../../format.js';
	import type { operatingContext } from '../../diagram/transmission-results.js';
	import {
		TRANSFER_SCALES,
		transferDirectionText,
		type TransferReading
	} from '../../diagram/transmission-transfer.js';
	import { transferLead } from '../../diagram/transmission-explain.js';
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
			missing,
			zero,
			top: first ? { id: first.branch.id, mw: readable(first.reading.magnitude, 'MW').text } : null
		});
	});
</script>

<section class="transfer" aria-label="Flows">
	<TxPanelHead title="Flows" {context} {lead} view="transfer" />
	<label>
		<span><i class="swatch"></i>Line width</span>
		{#if brief}0–{scale} MW{:else}<select
				aria-label="Width scale"
				value={scale}
				onchange={(event) => onscale(Number(event.currentTarget.value))}
			>
				{#each TRANSFER_SCALES as value (value)}<option {value}>0–{value} MW</option>{/each}
			</select>{/if}
	</label>
	{#if !brief}
		<details bind:open>
			<summary>Largest flows</summary>
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
								<strong>Line {branch.id}</strong><span
									>{readable(reading.magnitude, 'MW').text}, {transferDirectionText(
										reading,
										branch
									)}</span
								>
							</button>
						</li>
					{/each}
				</ol>
			{:else}<p>None</p>{/if}
		</details>{/if}
</section>

<style>
	.transfer {
		width: 100%;
		min-width: 0;
		font-family: var(--font-display);
		font-size: var(--fs-label);
		line-height: var(--lh-snug);
	}
	label {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: var(--space-2xs) var(--space-md);
	}
	strong {
		color: var(--ink);
	}
	label span {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}
	.swatch {
		width: 18px;
		height: 4px;
		border-radius: 2px;
		background: #1b6c97;
	}
	p {
		margin: var(--space-2xs) 0;
	}
	details {
		border-top: 1px solid var(--line);
		margin-top: var(--space-2xs);
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
