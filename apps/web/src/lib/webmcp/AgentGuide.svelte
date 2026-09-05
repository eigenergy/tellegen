<script lang="ts">
	import { resolve } from '$app/paths';
	let { supported, registrationError }: { supported: boolean; registrationError: string | null } =
		$props();
	const prompt =
		'Use the WebMCP tools in this Tellegen tab to inspect the network, explain congestion, and compare demand or capacity changes. Save the results in a Study. Ask before applying a proposal.';
	let copied = $state(false);
	let copyError = $state(false);
	async function copy() {
		try {
			await navigator.clipboard.writeText(prompt);
			copied = true;
			copyError = false;
		} catch {
			copyError = true;
		}
	}
</script>

<section aria-label="Use Tellegen with an agent">
	<p>Open this tab in your agent's browser, then send this request.</p>
	<label for="agent-prompt">Agent prompt</label>
	<textarea id="agent-prompt" rows="5" readonly value={prompt}></textarea>
	<div class="actions">
		<button onclick={copy}>{copied ? 'Copied' : 'Copy prompt'}</button>
		<a href="https://eigenergy.github.io/tellegen/webmcp.html" target="_blank" rel="noreferrer"
			>Setup guide</a
		>
		<a href={resolve('/changelog')}>Changelog</a>
	</div>
	{#if copyError}<p role="status">Select and copy the request above.</p>{/if}
	<p class="availability">
		<span class:available={supported && !registrationError}></span>{registrationError
			? 'Registration failed. Reload to retry.'
			: supported
				? 'WebMCP is ready in this tab.'
				: 'Requires a browser with WebMCP support.'}
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
		background: white;
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
	button {
		font: inherit;
		padding: 8px 12px;
		color: white;
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
