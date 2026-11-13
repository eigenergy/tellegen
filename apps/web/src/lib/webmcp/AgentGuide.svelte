<script lang="ts">
	import { resolve } from '$app/paths';
	let { supported, registrationError }: { supported: boolean; registrationError: string | null } =
		$props();
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
</script>

<section aria-label="Use Tellegen with an agent">
	<p>Open this tab in your agent's browser and copy this prompt.</p>
	<label for="agent-prompt">Agent prompt</label>
	<textarea id="agent-prompt" rows="6" readonly value={prompt}></textarea>
	<div class="actions">
		<button class="primary" onclick={() => copy(prompt)}
			>{copied === prompt ? 'Copied' : 'Copy prompt'}</button
		>
		<a href="https://eigenergy.github.io/tellegen/webmcp.html" target="_blank" rel="noreferrer"
			>Setup guide</a
		>
		<a href={resolve('/changelog')}>Changelog</a>
	</div>
	{#if copyError}<p role="status">Select the text and copy it yourself.</p>{/if}
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
	<p class="availability">
		<span class={{ available: supported && !registrationError }}></span>{registrationError
			? 'Registration failed. Reload to retry.'
			: supported
				? 'WebMCP tools available. Calls appear in Activity.'
				: 'WebMCP tools unavailable in this browser.'}
	</p>
</section>

<style>
	section {
		padding: 20px;
		font: 13px/1.5 var(--font-display);
	}
	p {
		margin: 0 0 16px;
	}
	label {
		display: block;
		margin-bottom: 6px;
		font-weight: 600;
	}
	textarea {
		width: 100%;
		box-sizing: border-box;
		padding: 12px;
		resize: vertical;
		border: 1px solid var(--line);
		border-radius: 4px;
		background: var(--paper);
		color: var(--ink);
		font: inherit;
		line-height: 1.6;
	}
	.actions {
		display: flex;
		align-items: center;
		gap: 16px;
		margin-top: 14px;
	}
	h3 {
		margin: 24px 0 8px;
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
	}
	.ask-copy:hover {
		border-color: var(--agent);
		color: var(--agent);
	}
	.primary {
		font: inherit;
		padding: 8px 12px;
		color: var(--paper);
		background: var(--ink);
		border: 1px solid var(--ink);
		border-radius: 4px;
		cursor: pointer;
	}
	a {
		color: var(--text-secondary);
		text-underline-offset: 3px;
		font-size: 12px;
	}
	.availability {
		margin: 20px 0 0;
		font-size: 11px;
		color: var(--text-secondary);
		display: flex;
		gap: 8px;
		align-items: center;
	}
	.availability span {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--ink-dim);
	}
	.availability span.available {
		background: var(--neg);
	}
</style>
