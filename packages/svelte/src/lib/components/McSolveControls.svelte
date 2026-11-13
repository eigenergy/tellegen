<script lang="ts">
	import type { Snippet } from 'svelte';
	import { validateMcPfOptions } from '@tellegen/engine';
	import { getNoticeCenter } from '../notices.svelte.js';
	import type { MulticonductorCase } from '../state.svelte.js';
	import { MC_PF_UNAVAILABLE, MC_PF_UNAVAILABLE_TITLE } from '../multiconductor.js';

	let {
		mc,
		busy = false,
		onsolve,
		onsettings,
		actions
	}: {
		/** The distribution case to solve. */
		mc: MulticonductorCase;
		/** Something else holds the case (a save or an import), so the run waits. */
		busy?: boolean;
		/** Run the multiconductor power flow; the caller chooses what follows it. */
		onsolve: () => void;
		/** Open the power flow settings. */
		onsettings: () => void;
		/** Further buttons for the same row, such as saving the result. */
		actions?: Snippet;
	} = $props();

	const notices = getNoticeCenter();
	const invalid = $derived(validateMcPfOptions(mc.mcOptions));
</script>

<div class="mc-solve">
	{#if !mc.mcPfSupported && mc.mcPfReason}<p class="note">{mc.mcPfReason}</p>{/if}
	{#if invalid}<p class="note">
			{invalid} Open the power flow settings to change it before solving.
		</p>{/if}
	<button class="quiet" onclick={onsettings}>Power flow settings</button>
	<div class="actions">
		{#if mc.solving}
			<span role="status">Calculating…</span>{#if mc.solveAbort}<button
					class="quiet"
					onclick={() => mc.solveAbort?.abort()}>Cancel</button
				>{/if}
		{:else}
			<button class="primary" disabled={!mc.mcPfSupported || busy || !!invalid} onclick={onsolve}
				>{mc.mcSolveLabel}</button
			>
		{/if}
		{@render actions?.()}
	</div>
	{#if !mc.mcPfSupported}<button
			class="text"
			onclick={() =>
				notices.push({
					kind: 'warning',
					title: MC_PF_UNAVAILABLE_TITLE,
					details: mc.mcPfReason ?? MC_PF_UNAVAILABLE
				})}>Why unavailable</button
		>{/if}
</div>

<style>
	.mc-solve {
		margin: 10px 0 14px;
	}
	.note {
		margin: 0 0 8px;
		color: var(--text-secondary);
		font-size: 11px;
		line-height: 1.5;
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		align-items: center;
		margin-top: 10px;
	}
	button {
		font: inherit;
		padding: 7px 11px;
		border: 1px solid var(--line);
		border-radius: 4px;
		background: transparent;
		color: var(--ink);
		cursor: pointer;
	}
	button:disabled {
		opacity: 0.45;
		cursor: default;
	}
	button.primary {
		background: var(--ink);
		border-color: var(--ink);
		color: var(--paper);
	}
	button.text {
		border: 0;
		padding: 6px 0;
		color: var(--text-secondary);
		font-size: 12px;
	}
	button:focus-visible {
		outline: 2px solid var(--focus-ring, var(--accent));
		outline-offset: 2px;
	}
</style>
