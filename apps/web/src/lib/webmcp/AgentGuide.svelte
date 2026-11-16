<script lang="ts">
	import { onMount } from 'svelte';
	import { WEBMCP_TRIAL_ORIGIN, type WebMcpBrowserState } from './browser-support.js';

	let {
		supported,
		registrationError,
		browser = null,
		pageToolCalls = true,
		toolCount = 0,
		callCount = 0
	}: {
		supported: boolean;
		registrationError: string | null;
		/** Null until the page has checked the browser. */
		browser?: WebMcpBrowserState | null;
		/** Whether this browser has getTools and executeTool, which Claude in Chrome uses. */
		pageToolCalls?: boolean;
		toolCount?: number;
		callCount?: number;
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
	/** Requests that show the tools at work on a solved DC OPF case. */
	const asks = [
		'Which lines are congested right now, and which buses pay the highest prices?',
		'If demand at the most expensive bus rises by 50 MW, how do prices and the total cost change? Preview it before applying anything.',
		'Propose the cheapest line capacity upgrade that relieves congestion, and wait for my approval before applying it.'
	];
	const devtoolsArgs =
		'npx chrome-devtools-mcp@latest --categoryExperimentalWebmcp=true --chromeArg=--enable-features=WebMCP';
	const commands = {
		claudeChrome: 'claude --chrome',
		claudeDevtools: `claude mcp add chrome-devtools --scope user -- ${devtoolsArgs}`,
		codexDevtools: `codex mcp add chrome-devtools -- ${devtoolsArgs}`
	};

	let origin = $state(WEBMCP_TRIAL_ORIGIN);
	// Claude in Chrome does not list page tools by itself yet, so the prompt
	// tells it where they are and how to call them: executeTool takes the tool
	// object from getTools and a plain input object (Chrome's Imperative API).
	const claudePrompt = $derived(
		`Open ${origin} in Chrome. The page offers WebMCP tools; use them instead of clicking. ` +
			'List them by running `const tools = await document.modelContext.getTools()` in the page ' +
			'(each has a name, description and inputSchema). Call one with ' +
			"`await document.modelContext.executeTool(tools.find((t) => t.name === '<name>'), { ...args })`, " +
			'passing the tool object and a plain object of arguments, not a name or a JSON string. ' +
			prompt
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

	const ready = $derived(supported && !registrationError);
	/** The browser by name: Chrome, Edge, or "this browser". */
	const brandName = (brand: 'Chrome' | 'Edge' | 'Chromium') =>
		brand === 'Chromium' ? 'this browser' : brand;
</script>

{#snippet copyRow(text: string, label: string)}
	<div class="cmd">
		<code title={text}>{text}</code>
		<button
			class="ask-copy"
			aria-label={copied === text ? `Copied ${label}` : `Copy ${label}`}
			onclick={() => copy(text)}>{copied === text ? 'Copied' : 'Copy'}</button
		>
	</div>
{/snippet}

<section aria-label="Use Tellegen with an agent">
	{#if registrationError}
		<div class="status" data-state="error" role="status" data-testid="webmcp-status">
			<strong title={registrationError}>Tools failed to load</strong>
			<details>
				<summary>Details</summary>
				<p>{registrationError}</p>
			</details>
			<button class="ask-copy" onclick={() => location.reload()}>Reload</button>
		</div>
	{:else if ready}
		<div class="status" data-state="ready" role="status" data-testid="webmcp-status">
			<strong><span class="dot"></span>{callCount > 0 ? 'Connected' : 'Ready'}</strong>
			<dl class="counts">
				<div>
					<dt>Tools</dt>
					<dd>{toolCount}</dd>
				</div>
				<div>
					<dt>Calls</dt>
					<dd>{callCount}</dd>
				</div>
			</dl>
		</div>
	{:else if browser?.kind === 'ready'}
		<div class="status" data-state="pending" role="status" data-testid="webmcp-status">
			<strong><span class="dot"></span>Loading…</strong>
		</div>
	{:else if browser}
		<div class="status" data-state="setup" role="status" data-testid="webmcp-status">
			{#if browser.kind === 'flag'}
				<strong>WebMCP off</strong>
				<p class="label">Enable at</p>
				{@render copyRow(browser.flagUrl, 'flag address')}
			{:else if browser.kind === 'update' || browser.kind === 'legacy'}
				<strong>Needs {brandName(browser.brand)} {browser.minVersion}+</strong>
			{:else if browser.kind === 'mobile'}
				<strong>Desktop only</strong>
			{:else if browser.kind === 'other'}
				<strong>Not supported in {browser.browser ?? 'this browser'}</strong>
			{/if}
		</div>
	{/if}

	<h3>Agents</h3>
	<div class="routes">
		<details open>
			<summary>ChatGPT app</summary>
			<p>Open {origin} in the app's browser</p>
		</details>
		<details>
			<summary>Claude in Chrome</summary>
			{@render copyRow(commands.claudeChrome, 'Claude Code command')}
			{#if ready && !pageToolCalls}
				<p class="hint" data-testid="page-tool-calls-missing">Use DevTools MCP</p>
			{:else}
				<button class="secondary" onclick={() => copy(claudePrompt)}
					>{copied === claudePrompt ? 'Copied' : 'Copy prompt for Claude'}</button
				>
			{/if}
		</details>
		<details>
			<summary>DevTools MCP</summary>
			<p class="label">Claude Code</p>
			{@render copyRow(commands.claudeDevtools, 'Claude Code DevTools command')}
			<p class="label">Codex CLI</p>
			{@render copyRow(commands.codexDevtools, 'Codex DevTools command')}
		</details>
	</div>

	{#if ready}
		<h3>Prompt</h3>
		<div class="actions">
			<button class="secondary" onclick={() => copy(prompt)}
				>{copied === prompt ? 'Copied' : 'Copy prompt'}</button
			>
		</div>
		<h3>Examples</h3>
		<ul class="asks">
			{#each asks as ask (ask)}
				<li>
					<span>{ask}</span>
					<button
						class="ask-copy"
						aria-label={copied === ask ? `Copied: ${ask}` : `Copy: ${ask}`}
						onclick={() => copy(ask)}>{copied === ask ? 'Copied' : 'Copy'}</button
					>
				</li>
			{/each}
		</ul>
	{/if}
	{#if copyError}<p role="status" class="hint">Copy failed</p>{/if}
	<p class="visually-hidden" role="status">{copied ? 'Copied' : ''}</p>

	<div class="actions links">
		<a href="{docs}/webmcp.html#getting-started" target="_blank" rel="noreferrer">Docs</a>
	</div>
</section>

<style>
	section {
		padding: 16px 20px 20px;
		font: 13px/1.5 var(--font-display);
	}
	p {
		margin: 0 0 12px;
	}
	.hint {
		color: var(--text-secondary);
		font-size: 12px;
	}
	.status {
		margin-bottom: 16px;
		padding: 10px 12px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		background: var(--raised);
	}
	.status[data-state='error'] strong {
		color: var(--text-danger);
	}
	.status strong {
		display: flex;
		gap: 8px;
		align-items: center;
		margin-bottom: 4px;
	}
	.status p {
		margin: 0 0 6px;
	}
	.status p:last-child {
		margin-bottom: 0;
	}
	.status .cmd {
		margin-bottom: 6px;
	}
	.visually-hidden {
		position: absolute;
		width: 1px;
		height: 1px;
		margin: -1px;
		padding: 0;
		overflow: hidden;
		clip-path: inset(50%);
		white-space: nowrap;
		border: 0;
	}
	.status[data-state='pending'] .dot {
		background: var(--ink-dim);
	}
	.dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--agent);
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
	.label {
		display: block;
		margin: 0 0 6px;
		font-weight: 600;
	}
	.label {
		font-size: 12px;
		margin-top: 8px;
	}
	.counts {
		display: flex;
		gap: 24px;
		margin: 6px 0 0;
	}
	.counts dt {
		font-size: 12px;
		color: var(--text-secondary);
	}
	.counts dd {
		margin: 0;
		font-size: 18px;
		font-weight: 600;
		font-variant-numeric: tabular-nums;
	}
	.actions {
		display: flex;
		align-items: center;
		gap: 16px;
		margin-top: 10px;
	}
	h3 {
		margin: 20px 0 8px;
		font-size: 13px;
		font-weight: 600;
	}
	.asks {
		margin: 0;
		padding: 0;
		list-style: none;
		border-top: 1px solid var(--line);
	}
	.asks li {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: 12px;
		align-items: start;
		padding: 10px 0;
		border-bottom: 1px solid var(--line);
		font-size: 13px;
		line-height: 1.45;
	}
	.ask-copy {
		padding: 2px 8px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		background: transparent;
		color: var(--text-secondary);
		font: 12px/1.4 var(--font-display);
		cursor: pointer;
		flex: none;
	}
	.ask-copy:hover {
		border-color: var(--border-strong);
		color: var(--ink);
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
		display: flex;
		gap: 8px;
		align-items: center;
		margin-bottom: 8px;
	}
	.cmd code {
		flex: 1;
		min-width: 0;
		padding: 5px 8px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		background: var(--paper);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: 12px;
	}
	/* Monospace only for text to paste: the flag address and terminal commands.
	   Names mentioned in a sentence stay in the text face. */
	code {
		font: inherit;
		font-weight: 500;
	}
	.cmd code {
		font: 12px/1.45 var(--font-code);
	}
	.routes {
		border-top: 1px solid var(--line);
	}
	.routes details {
		padding: 8px 0;
		border-bottom: 1px solid var(--line);
	}
	.routes details[open] summary {
		margin-bottom: 6px;
	}
	a {
		color: var(--text-accent);
		text-decoration: none;
	}
	a:hover {
		text-decoration: underline;
		text-underline-offset: 3px;
	}
	.links a {
		font-size: 12px;
	}
</style>
