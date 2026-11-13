<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import type { WebMcpBrowserState } from './browser-support.js';

	let {
		supported,
		registrationError,
		browser = null,
		toolCount = 0,
		callCount = 0
	}: {
		supported: boolean;
		registrationError: string | null;
		/** Null until the page has checked the browser. */
		browser?: WebMcpBrowserState | null;
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

	let origin = $state('https://tellegen.dev');
	// Claude in Chrome does not list page tools by itself yet, so the prompt
	// tells it where they are.
	const claudePrompt = $derived(
		`Open ${origin} in Chrome. The page offers WebMCP tools: list them by running ` +
			'`await document.modelContext.getTools()` in the page, and call them with ' +
			'`document.modelContext.executeTool()` instead of clicking. ' +
			prompt
	);

	const introKey = 'tellegen.webmcp.intro-closed';
	let introOpen = $state(true);
	onMount(() => {
		origin = location.origin;
		try {
			introOpen = localStorage.getItem(introKey) !== '1';
		} catch {
			/* Storage blocked: keep the explainer open. */
		}
	});
	function rememberIntro(event: Event) {
		introOpen = (event.currentTarget as HTMLDetailsElement).open;
		try {
			if (introOpen) localStorage.removeItem(introKey);
			else localStorage.setItem(introKey, '1');
		} catch {
			/* The choice applies to this visit. */
		}
	}

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
	const readyText = $derived(
		callCount > 0
			? `Your agent has made ${callCount} ${callCount === 1 ? 'call' : 'calls'}. Watch them in Activity.`
			: `This tab offers ${toolCount || 'its'} ${toolCount === 1 ? 'tool' : 'tools'} to an AI agent. Connect one below, then ask it something. Calls appear in Activity.`
	);
	const brandName = (brand: 'Chrome' | 'Edge' | 'Chromium') =>
		brand === 'Chromium' ? 'This browser' : brand;
</script>

{#snippet copyRow(text: string, label: string)}
	<div class="cmd">
		<code>{text}</code>
		<button class="ask-copy" aria-label="Copy {label}" onclick={() => copy(text)}
			>{copied === text ? 'Copied' : 'Copy'}</button
		>
	</div>
{/snippet}

<section aria-label="Use Tellegen with an agent">
	{#if registrationError}
		<div class="status" data-state="error" role="status" data-testid="webmcp-status">
			<strong>The tools didn't load</strong>
			<p>
				The browser turned Tellegen's tools away: {registrationError}. Reload to try again. If it
				keeps happening, open the page in a new Chrome window.
			</p>
		</div>
	{:else if ready}
		<div class="status" data-state="ready" role="status" data-testid="webmcp-status">
			<strong
				><span class="dot"></span>{callCount > 0 ? 'Agent connected' : 'Ready for agents'}</strong
			>
			<p>{readyText}</p>
			{#if browser?.kind === 'ready' && browser.mobile}
				<p>The agents that use these tools run on a computer for now.</p>
			{/if}
		</div>
	{:else if browser}
		<div class="status" data-state="setup" role="status" data-testid="webmcp-status">
			{#if browser.kind === 'flag'}
				<strong>One switch away</strong>
				<p>
					{brandName(browser.brand)}{browser.version ? ` ${browser.version}` : ''} can run WebMCP, but
					it's off for this page. Paste this into the address bar, set it to Enabled, then relaunch:
				</p>
				{@render copyRow(browser.flagUrl, 'flag address')}
				<p class="hint">
					{browser.brand === 'Chrome'
						? 'Chrome 149 and later need no flag on tellegen.dev itself; local and preview copies of the site do.'
						: 'Chrome needs no flag on tellegen.dev, if you have it.'}
				</p>
			{:else if browser.kind === 'update' || browser.kind === 'legacy'}
				<strong>Almost there</strong>
				<p>
					{brandName(browser.brand)}{browser.version ? ` ${browser.version}` : ''} is a little too old
					for WebMCP. Update to version 149 or later (Settings, then About), and reload.
				</p>
			{:else if browser.kind === 'mobile'}
				<strong>Agents work from a computer</strong>
				<p>
					The agents that use these tools run on desktop for now. Open tellegen.dev in Chrome on a
					computer. The map works here as usual.
				</p>
			{:else if browser.kind === 'other'}
				<strong>{browser.browser ?? 'This browser'} doesn't speak WebMCP yet</strong>
				<p>
					WebMCP is new enough that only Chrome and Edge know it so far. Open this page in Chrome on
					a computer, or use an agent below that brings its own browser.
				</p>
			{/if}
		</div>
	{/if}

	<details class="intro" open={introOpen} ontoggle={rememberIntro}>
		<summary>What is WebMCP?</summary>
		<p>
			WebMCP lets a web page hand your AI assistant a toolbox. Instead of squinting at the screen
			and clicking, the agent calls Tellegen's tools directly, and every call shows up here.
		</p>
		<div class="toolbox" aria-hidden="true">
			<span class="who">Your agent</span>
			<span class="arrow">→</span>
			<span class="box">
				<span>inspect_case</span><span>query_network</span><span>update_case</span>
			</span>
			<span class="arrow">→</span>
			<span class="who">This map</span>
		</div>
		<p class="hint">
			It's a new browser standard, in trial in Chrome and Edge. For most people this is a first.
		</p>
	</details>

	{#if ready}
		<label for="agent-prompt">Agent prompt</label>
		<textarea id="agent-prompt" rows="5" readonly value={prompt}></textarea>
		<div class="actions">
			<button class="primary" onclick={() => copy(prompt)}
				>{copied === prompt ? 'Copied' : 'Copy prompt'}</button
			>
		</div>
		<h3>Things to ask</h3>
		<ul class="asks">
			{#each asks as ask (ask)}
				<li>
					<span>{ask}</span>
					<button class="ask-copy" aria-label="Copy: {ask}" onclick={() => copy(ask)}
						>{copied === ask ? 'Copied' : 'Copy'}</button
					>
				</li>
			{/each}
		</ul>
	{/if}
	{#if copyError}<p role="status" class="hint">Select the text and copy it yourself.</p>{/if}

	<h3>Connect your agent</h3>
	<div class="routes">
		<details open>
			<summary>ChatGPT desktop app <span class="tag">No setup</span></summary>
			<ol>
				<li>Open {origin} in the app's built-in browser.</li>
				<li>Ask for what you want. The agent finds the tools itself.</li>
				<li>The <b>Site tools</b> button in the address bar lists them.</li>
			</ol>
			<p class="hint">
				Not in the Codex CLI or IDE extension, and not through the Chrome connector.
			</p>
		</details>
		<details>
			<summary>Claude Code with Claude in Chrome</summary>
			<ol>
				<li>Install the Claude in Chrome extension and sign in to Claude Code with /login.</li>
				<li>Start Claude Code with Chrome:</li>
			</ol>
			{@render copyRow(commands.claudeChrome, 'Claude Code command')}
			<p>
				Claude doesn't look for page tools on its own yet, so start with this prompt. It tells
				Claude where the toolbox is.
			</p>
			<button class="primary" onclick={() => copy(claudePrompt)}
				>{copied === claudePrompt ? 'Copied' : 'Copy prompt for Claude'}</button
			>
		</details>
		<details>
			<summary>Claude Code, Codex CLI or Cursor with Chrome DevTools MCP</summary>
			<p>
				Add Google's Chrome DevTools MCP server once. It opens its own Chrome window with WebMCP on
				and gives the agent <code>list_webmcp_tools</code> and <code>execute_webmcp_tool</code>.
			</p>
			<p class="label">Claude Code</p>
			{@render copyRow(commands.claudeDevtools, 'Claude Code DevTools command')}
			<p class="label">Codex CLI</p>
			{@render copyRow(commands.codexDevtools, 'Codex DevTools command')}
			<p>Then ask the agent to open {origin} and list its WebMCP tools.</p>
		</details>
		<details>
			<summary>Agents built into the browser</summary>
			<p>
				Gemini in Chrome and Copilot in Edge don't call page tools yet. When they do, opening this
				page will be all it takes.
			</p>
		</details>
	</div>

	<h3>Questions</h3>
	<dl class="faq">
		<dt>Do I need to turn on remote debugging?</dt>
		<dd>
			No. You only need it to attach Chrome DevTools MCP to the Chrome you already have open (its
			<code>--autoConnect</code> option). None of the setups above do that.
		</dd>
		<dt>Do I need to add an MCP server?</dt>
		<dd>
			Only for the Chrome DevTools MCP route, with the one command above. The ChatGPT app and
			<code>claude --chrome</code> need none. To work on case files without a browser, use
			<a href="{docs}/mcp.html" target="_blank" rel="noreferrer">tellegen-mcp</a>.
		</dd>
	</dl>

	<div class="actions links">
		<a href="{docs}/webmcp.html#getting-started" target="_blank" rel="noreferrer">Setup guide</a>
		<a href={resolve('/changelog')}>Changelog</a>
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
		border-left: 3px solid var(--ink-dim);
		border-radius: 4px;
		background: var(--paper);
	}
	.status[data-state='ready'] {
		border-left-color: var(--agent);
		background: var(--agent-soft);
	}
	.status[data-state='setup'] {
		border-left-color: var(--accent);
	}
	.status[data-state='error'] {
		border-left-color: var(--text-danger);
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
	.intro {
		margin-bottom: 16px;
		padding-bottom: 12px;
		border-bottom: 1px solid var(--line);
	}
	.intro p {
		margin: 6px 0 10px;
	}
	.toolbox {
		display: flex;
		align-items: center;
		gap: 6px;
		margin: 0 0 10px;
		font-size: 11px;
		color: var(--text-secondary);
	}
	.toolbox .who {
		white-space: nowrap;
	}
	.toolbox .arrow {
		color: var(--agent);
	}
	.toolbox .box {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: 4px 6px;
		border: 1px dashed rgb(var(--agent-rgb) / 0.6);
		border-radius: 4px;
		background: var(--agent-soft);
		color: var(--agent);
		font: 10px/1.3 var(--font-mono);
	}
	label,
	.label {
		display: block;
		margin: 0 0 6px;
		font-weight: 600;
	}
	.label {
		font-size: 12px;
		margin-top: 8px;
	}
	textarea {
		width: 100%;
		box-sizing: border-box;
		padding: 10px 12px;
		resize: vertical;
		border: 1px solid var(--line);
		border-radius: 4px;
		background: var(--paper);
		color: var(--ink);
		font: inherit;
		line-height: 1.55;
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
		font-size: 12.5px;
		line-height: 1.45;
	}
	.ask-copy {
		padding: 2px 8px;
		border: 1px solid var(--line);
		border-radius: 3px;
		background: transparent;
		color: var(--text-secondary);
		font: 11px/1.4 var(--font-display);
		cursor: pointer;
		flex: none;
	}
	.ask-copy:hover {
		border-color: var(--agent);
		color: var(--agent);
	}
	.primary {
		font: inherit;
		padding: 7px 12px;
		color: var(--paper);
		background: var(--ink);
		border: 1px solid var(--ink);
		border-radius: 4px;
		cursor: pointer;
	}
	.cmd {
		display: flex;
		gap: 8px;
		align-items: flex-start;
		margin-bottom: 8px;
	}
	.cmd code {
		flex: 1;
		min-width: 0;
		padding: 5px 8px;
		border: 1px solid var(--line);
		border-radius: 4px;
		background: var(--paper);
		overflow-wrap: anywhere;
		font-size: 11px;
	}
	code {
		font: 11px/1.45 var(--font-mono);
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
	.routes ol {
		margin: 0 0 8px;
		padding-left: 18px;
	}
	.routes li {
		margin-bottom: 2px;
	}
	.tag {
		margin-left: 6px;
		padding: 1px 6px;
		border-radius: 3px;
		background: var(--agent-soft);
		color: var(--agent);
		font-size: 10.5px;
		font-weight: 500;
	}
	.faq {
		margin: 0;
	}
	.faq dt {
		font-weight: 600;
	}
	.faq dd {
		margin: 2px 0 12px;
		color: var(--text-secondary);
	}
	a {
		color: var(--text-secondary);
		text-underline-offset: 3px;
	}
	.links a {
		font-size: 12px;
	}
</style>
