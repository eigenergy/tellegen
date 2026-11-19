<script lang="ts">
	/** A controller graph full screen: the block canvas with its palette and
	 * form, the engine's check, and the actions that put the graph into the
	 * case or take it out again.
	 * @block Controller graph editor */
	import { GRAPH_PORTS, type BlockGraph } from '@tellegen/engine';
	import GraphCanvas from './GraphCanvas.svelte';
	import { getUiConfig } from '../../context.svelte.js';

	const config = getUiConfig();
	/** Mount the dialog on the provider's root, above the panel dock and the
	 * header: the theme tokens live there, so it keeps them. */
	function toRoot(node: HTMLElement) {
		(config.element ?? document.body).appendChild(node);
		return { destroy: () => node.remove() };
	}

	let {
		graph: initial,
		title,
		values = null,
		onapply,
		onrevert,
		onclose
	}: {
		graph: BlockGraph;
		title: string;
		/** Block outputs at the shown instant (live run or playback). */
		values?: Record<string, number> | null;
		/** Use the graph in the case (initializes again); throws when it does not initialize. */
		onapply: (graph: BlockGraph) => Promise<void>;
		/** Go back to the DYR record (shown when the case uses a graph). */
		onrevert?: () => Promise<void>;
		onclose: () => void;
	} = $props();

	// The editor works on its own copy of the graph it was opened with.
	// svelte-ignore state_referenced_locally
	let g = $state<BlockGraph>(JSON.parse(JSON.stringify(initial)));
	let check = $state<{ ok: true; states: string[] } | { ok: false; error: string } | null>(null);
	let busy = $state(false);
	let applyError = $state<string | null>(null);

	// Escape closes the editor, except while a wire is being drawn (the
	// canvas cancels the wire) or a field has the focus (it leaves the field).
	let wiring = $state(false);
	function onkey(e: KeyboardEvent) {
		if (e.key !== 'Escape' || wiring || e.defaultPrevented) return;
		const t = e.target;
		if (
			t instanceof HTMLInputElement ||
			t instanceof HTMLSelectElement ||
			t instanceof HTMLTextAreaElement ||
			(t instanceof HTMLElement && t.isContentEditable)
		)
			return;
		onclose();
	}

	async function run(action: () => Promise<void>) {
		busy = true;
		applyError = null;
		try {
			await action();
		} catch (e) {
			applyError = e instanceof Error ? e.message : String(e);
		} finally {
			busy = false;
		}
	}
</script>

<svelte:window onkeydown={onkey} />

<div class="editor dyn-form" role="dialog" aria-label={title} use:toRoot>
	<header>
		<h3>{title}</h3>
		<input class="name mono" bind:value={g.name} aria-label="graph name" />
		<span class="dim">{g.slot} · ports {GRAPH_PORTS[g.slot].join(', ')}</span>
		<span class="grow"></span>
		{#if check}
			{#if check.ok}
				<span class="ok">✓ {check.states.length} state{check.states.length === 1 ? '' : 's'}</span>
			{:else}
				<span class="bad" title={check.error}>✗ {check.error}</span>
			{/if}
		{/if}
		{#if onrevert}<button class="ghost" disabled={busy} onclick={() => onrevert && run(onrevert)}>Back to DYR record</button>{/if}
		<button disabled={busy || !check?.ok} onclick={() => run(() => onapply(JSON.parse(JSON.stringify(g))))}>
			{busy ? 'Initializing…' : 'Use in case'}
		</button>
		<button class="ghost" onclick={onclose}>Close</button>
	</header>
	{#if applyError}<p class="bad msg">{applyError}</p>{/if}
	<GraphCanvas bind:graph={g} bind:check bind:wiring {values} />
</div>

<style>
	.editor {
		position: fixed;
		left: 24px;
		right: 24px;
		top: 64px;
		bottom: 24px;
		z-index: 60;
		display: flex;
		flex-direction: column;
		background: var(--surface-menu);
		border: 1px solid var(--border-default);
		border-radius: 8px;
		box-shadow: 0 8px 28px rgb(0 0 0 / 0.22);
		color: var(--text-primary);
		font-size: var(--fs-control);
	}
	header {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 8px 12px;
		border-bottom: 1px solid var(--border-default);
		flex-wrap: wrap;
	}
	h3 {
		margin: 0;
		font-size: var(--fs-label);
	}
	.name {
		width: 120px;
	}
	.grow {
		flex: 1;
	}
	.mono {
		font-family: var(--font-mono);
	}
	.dim {
		color: var(--text-tertiary);
	}
	.ok {
		color: var(--text-accent);
	}
	.bad {
		color: var(--text-danger);
		max-width: 46ch;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.msg {
		margin: 4px 12px;
		white-space: normal;
		max-width: none;
	}
</style>
