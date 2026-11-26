<script lang="ts">
	import { onMount } from 'svelte';
	import { StatBoxes } from '@tellegen/svelte';
	import { WEBMCP_TRIAL_ORIGIN, type WebMcpBrowserState } from './browser-support.js';
	import { connectStatus } from './connect-status.js';

	let {
		supported,
		registrationError,
		browser = null,
		pageToolCalls = true,
		toolCount = 0,
		callCount = 0,
		unavailableTools = []
	}: {
		supported: boolean;
		registrationError: string | null;
		/** Null until the page has checked the browser. */
		browser?: WebMcpBrowserState | null;
		/** Whether this browser has getTools and executeTool, which Claude in Chrome uses. */
		pageToolCalls?: boolean;
		toolCount?: number;
		callCount?: number;
		/** Tools this tab does not offer right now, each with the reason. */
		unavailableTools?: readonly { name: string; reason: string }[];
	} = $props();

	const docs = 'https://eigenergy.github.io/tellegen';
	const prompt = [
		'Use the WebMCP tools in this Tellegen tab to work with the power grid it shows.',
		'Start with inspect_case, then use query_network to look up buses and lines; focus_network shows an element on the map.',
		'Switch networks with list_cases and select_case.',
		'Before you change demand or line ratings, call preview_case_update and tell me the predicted effect. Apply a change with update_case only when I ask.',
		'A capacity proposal needs my approval in the Agent panel before you apply it. Save a Study only when I ask.',
		'If the tools are unavailable, tell me so instead of asking to record the screen.'
	].join(' ');

	let origin = $state(WEBMCP_TRIAL_ORIGIN);
	// Claude in Chrome does not list page tools by itself yet, so where the browser
	// has getTools and executeTool the prompt says how to call them: executeTool
	// takes the tool object from getTools and a plain input object (Chrome's
	// Imperative API). Agents that list page tools themselves ignore the lead-in.
	const copiedPrompt = $derived(
		pageToolCalls
			? `Open ${origin} in Chrome. The page offers WebMCP tools; use them instead of clicking. ` +
					'List them by running `const tools = await document.modelContext.getTools()` in the page ' +
					'(each has a name, description and inputSchema). Call one with ' +
					"`await document.modelContext.executeTool(tools.find((t) => t.name === '<name>'), { ...args })`, " +
					'passing the tool object and a plain object of arguments, not a name or a JSON string. ' +
					prompt
			: prompt
	);

	onMount(() => {
		origin = location.origin;
	});

	let copied = $state<string | null>(null);
	let copyError = $state(false);
	async function copy(text: string) {
		try {
			await navigator.clipboard.writeText(text);
			copied = text;
			copyError = false;
		} catch {
			copyError = true;
		}
	}

	const status = $derived(connectStatus({ supported, registrationError, browser }));
	const ready = $derived(status.id === 'connected');
</script>

<section aria-label="Use Tellegen with an agent">
	<!-- One card: the status, what to do about it, and the docs. -->
	<div class="card" data-state={status.id === 'error' ? 'error' : ready ? 'ready' : 'setup'}>
		<div class="status" role="status" data-testid="webmcp-status">
			<strong class:pending={status.id === 'loading'} title={registrationError}>
				{#if ready || status.id === 'loading'}<span class="dot"></span>{/if}{status.label}
			</strong>
			{#if registrationError}
				<details>
					<summary>Details</summary>
					<p>{registrationError}</p>
				</details>
			{/if}
		</div>
		<div class="actions">
			{#if status.id === 'error'}
				<button class="secondary" onclick={() => location.reload()}>Reload</button>
			{:else if ready}
				<button class="secondary" onclick={() => copy(copiedPrompt)}>
					{copied === copiedPrompt ? 'Copied' : 'Copy prompt'}
				</button>
			{/if}
			<a href="{docs}/webmcp.html#getting-started" target="_blank" rel="noreferrer">Docs</a>
		</div>
		{#if status.next && status.id !== 'error'}
			<p class="hint" data-testid="webmcp-next">{status.next}</p>
		{/if}
		{#if status.flagUrl}
			{@const flagUrl = status.flagUrl}
			<div class="cmd">
				<code>{flagUrl}</code>
				<button
					class="cmd-copy"
					aria-label={copied === flagUrl ? 'Copied flag address' : 'Copy flag address'}
					onclick={() => copy(flagUrl)}
				>
					{copied === flagUrl ? 'Copied' : 'Copy'}
				</button>
			</div>
		{/if}
		{#if copyError}<p role="status" class="hint">Copy failed</p>{/if}
	</div>
	{#if ready}
		<div class="counts">
			<StatBoxes
				label="Agent tools"
				stats={[
					{ label: 'Tools', value: toolCount.toLocaleString('en-US') },
					{ label: 'Calls', value: callCount.toLocaleString('en-US') }
				]}
			/>
		</div>
	{/if}
	{#if ready && unavailableTools.length}
		<dl class="unavailable" aria-label="Tools not offered now">
			{#each unavailableTools as tool (tool.name)}
				<dt><code>{tool.name}</code></dt>
				<dd>{tool.reason}</dd>
			{/each}
		</dl>
	{/if}
	<p class="sr-only" role="status">{copied ? 'Copied' : ''}</p>
</section>

<style>
	section {
		padding: 16px 18px;
		font: 13px/1.5 var(--font-display);
	}
	p {
		margin: 0;
	}
	/* Status on the left, its actions on the right, in one card. */
	.card {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: start;
		gap: 8px 12px;
		padding: 10px 12px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		background: var(--raised);
	}
	.hint {
		grid-column: 1 / -1;
		color: var(--text-secondary);
		font-size: 12px;
	}
	.status {
		min-width: 0;
	}
	.card[data-state='error'] strong {
		color: var(--text-danger);
	}
	.status strong {
		display: flex;
		gap: 8px;
		align-items: center;
		min-height: 24px;
	}
	.status p {
		margin: 0 0 6px;
	}
	.pending .dot {
		background: var(--ink-dim);
	}
	.dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		/* Green, as a call that succeeded in Activity. */
		background: var(--neg);
	}
	summary {
		cursor: pointer;
		font-weight: 600;
		padding: 2px 0;
	}
	summary:focus-visible,
	button:focus-visible,
	a:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
	}
	.counts :global(dl.stats) {
		margin: 12px 0 0;
	}
	/* One row per tool: its name, then why it is not offered. */
	.unavailable {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		gap: 4px 12px;
		margin: 12px 0 0;
		font-size: 12px;
	}
	.unavailable dt code {
		font: 11.5px/1.45 var(--font-code);
	}
	.unavailable dd {
		margin: 0;
		color: var(--text-secondary);
	}
	.actions {
		display: flex;
		align-items: center;
		gap: 12px;
		min-height: 24px;
	}
	.actions a {
		font-size: 12px;
	}
	.secondary {
		font: inherit;
		font-size: 12px;
		padding: 4px 10px;
		color: var(--ink);
		background: var(--surface-control);
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		cursor: pointer;
	}
	.secondary:hover {
		border-color: var(--border-strong);
	}
	.cmd {
		grid-column: 1 / -1;
		display: flex;
		align-items: stretch;
		height: 28px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		background: var(--paper);
		overflow: hidden;
	}
	/* Monospace only for the text to paste. */
	.cmd code {
		font: 11.5px/1.45 var(--font-code);
		flex: 1;
		min-width: 0;
		align-self: center;
		padding: 0 6px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.cmd-copy {
		flex: none;
		padding: 0 8px;
		border: 0;
		border-left: 1px solid var(--line);
		background: var(--surface-control);
		color: var(--text-secondary);
		font: 12px/1 var(--font-display);
		cursor: pointer;
	}
	.cmd-copy:hover {
		color: var(--ink);
		background: var(--surface-hover);
	}
	.cmd-copy:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: -2px;
	}
	a {
		color: var(--text-accent);
		text-decoration: none;
	}
	a:hover {
		text-decoration: underline;
		text-underline-offset: 3px;
	}
</style>
