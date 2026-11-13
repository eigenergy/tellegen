<script lang="ts">
	import { tick } from 'svelte';
	import { getAppState } from '../context.svelte.js';
	import {
		commandGroup,
		commandHint,
		commandTitle,
		getCommandRegistry
	} from '../commands.svelte.js';

	const app = getAppState();
	const commands = getCommandRegistry();
	const matches = $derived(commands.matches());

	// Writable derived: arrow keys move it, and a new result set resets it to the top.
	let highlighted = $derived.by(() => {
		void matches;
		return 0;
	});
	// Focus the input while the palette is open, then hand focus back to
	// whatever had it before, if that element is still on the page.
	function focusWhileOpen(element: HTMLInputElement) {
		const restoreTo = document.activeElement;
		element.focus();
		return () => {
			if (restoreTo instanceof HTMLElement && restoreTo.isConnected) restoreTo.focus();
		};
	}

	function onWindowKeydown(e: KeyboardEvent) {
		if (e.defaultPrevented) return;
		if ((e.metaKey || e.ctrlKey) && !e.altKey && e.key.toLowerCase() === 'k') {
			e.preventDefault();
			commands.toggle();
			return;
		}
		if (e.key !== 'Escape') return;
		if (commands.open) {
			e.preventDefault();
			commands.hide();
		} else if (app.zen) {
			e.preventDefault();
			app.zen = false;
		}
	}

	function onInputKeydown(e: KeyboardEvent) {
		if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
			e.preventDefault();
			if (matches.length === 0) return;
			const step = e.key === 'ArrowDown' ? 1 : -1;
			highlighted = (highlighted + step + matches.length) % matches.length;
			const id = `command-${matches[highlighted].id}`;
			void tick().then(() => document.getElementById(id)?.scrollIntoView({ block: 'nearest' }));
		} else if (e.key === 'Enter') {
			e.preventDefault();
			const command = matches[highlighted];
			if (command) commands.run(command);
		} else if (e.key === 'Tab') {
			// The input is the only focusable control; keep focus inside the dialog.
			e.preventDefault();
		}
	}
</script>

<svelte:window onkeydown={onWindowKeydown} />

{#if commands.open}
	<div
		class="backdrop"
		role="presentation"
		onpointerdown={(e) => {
			if (e.target === e.currentTarget) commands.hide();
		}}
	>
		<div class="palette" role="dialog" aria-modal="true" aria-label="Command palette">
			<input
				{@attach focusWhileOpen}
				bind:value={commands.query}
				type="text"
				role="combobox"
				aria-expanded="true"
				aria-controls="command-palette-options"
				aria-activedescendant={matches[highlighted]
					? `command-${matches[highlighted].id}`
					: undefined}
				aria-autocomplete="list"
				autocomplete="off"
				spellcheck="false"
				placeholder="Type a command…"
				onkeydown={onInputKeydown}
			/>
			<div id="command-palette-options" class="options" role="listbox" aria-label="Commands">
				{#each matches as command, index (command.id)}
					{@const hint = commandHint(command)}
					{@const group = commandGroup(command)}
					{#if index === 0 || commandGroup(matches[index - 1]) !== group}
						<p class="group" role="presentation">{group}</p>
					{/if}
					<button
						type="button"
						tabindex="-1"
						id={`command-${command.id}`}
						role="option"
						aria-selected={index === highlighted}
						class={['option', { highlighted: index === highlighted }]}
						onpointerenter={() => (highlighted = index)}
						onpointerdown={(e) => e.preventDefault()}
						onclick={() => commands.run(command)}
					>
						<span class="title">{commandTitle(command)}</span>
						{#if hint}<span class="hint">{hint}</span>{/if}
					</button>
				{:else}
					<p class="empty">No command matches “{commands.query}”.</p>
				{/each}
			</div>
			<p class="keys" aria-hidden="true">
				<span><kbd>↑</kbd><kbd>↓</kbd> Move</span>
				<span><kbd>Enter</kbd> Run</span>
				<span><kbd>Esc</kbd> Close</span>
			</p>
		</div>
	</div>
{/if}

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		z-index: 70;
		background: rgb(var(--ink-rgb) / 0.22);
		backdrop-filter: blur(3px);
		display: flex;
		justify-content: center;
		align-items: flex-start;
		padding: 14vh 16px 16px;
	}
	.palette {
		width: 100%;
		max-width: 560px;
		overflow: hidden;
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: 8px;
		box-shadow: var(--elev-menu);
		font: var(--fs-body) / var(--lh-tight, 1.2) var(--font-display);
		color: var(--ink);
		animation: rise var(--dur-fast) ease-out;
	}
	@keyframes rise {
		from {
			opacity: 0;
			transform: translateY(-6px);
		}
	}
	input {
		display: block;
		width: 100%;
		box-sizing: border-box;
		padding: 16px 18px;
		font: inherit;
		font-size: var(--fs-title);
		color: inherit;
		background: transparent;
		border: 0;
		border-bottom: 1px solid var(--line);
		border-radius: 0;
		outline: none;
	}
	input::placeholder {
		color: var(--ink-faint);
	}
	.options {
		padding: var(--space-xs, 4px) var(--space-sm, 8px) var(--space-sm, 8px);
		max-height: min(52vh, 440px);
		overflow: auto;
		overscroll-behavior: contain;
	}
	.group {
		margin: 0;
		padding: 10px 10px 4px;
		font-size: var(--fs-label);
		font-weight: 600;
		color: var(--text-secondary);
	}
	.option {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 2px;
		width: 100%;
		padding: 8px 10px;
		border: 0;
		border-left: 2px solid transparent;
		border-radius: 4px;
		background: transparent;
		color: inherit;
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.option.highlighted {
		background: var(--accent-soft);
		border-left-color: var(--accent);
	}
	.empty {
		margin: 0;
		padding: 12px 10px;
		color: var(--text-secondary);
	}
	.hint {
		font-size: var(--fs-label);
		color: var(--text-secondary);
	}
	.keys {
		display: flex;
		gap: var(--space-lg, 14px);
		margin: 0;
		padding: 8px 18px;
		border-top: 1px solid var(--line);
		font-size: var(--fs-label);
		color: var(--text-secondary);
	}
	kbd {
		font-family: var(--font-mono);
		font-size: var(--fs-meta);
		padding: 1px 4px;
		margin-right: 3px;
		border: 1px solid var(--line);
		border-radius: var(--radius-sm);
		background: var(--surface-control, transparent);
	}
	@media (max-width: 600px) {
		.keys {
			display: none;
		}
	}
</style>
