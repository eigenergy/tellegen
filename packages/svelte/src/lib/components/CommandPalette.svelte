<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { getAppState } from '../context.svelte.js';
	import { commandHint, commandTitle, getCommandRegistry } from '../commands.svelte.js';

	const app = getAppState();
	const commands = getCommandRegistry();
	const matches = $derived(commands.matches());

	let highlighted = $state(0);
	let input = $state<HTMLInputElement | null>(null);
	let restoreTo: Element | null = null;

	$effect(() => {
		// Reset the highlight whenever the result set changes.
		void matches;
		highlighted = 0;
	});

	$effect(() => {
		if (commands.open) {
			restoreTo = document.activeElement;
			void tick().then(() => input?.focus());
		} else if (restoreTo instanceof HTMLElement && restoreTo.isConnected) {
			restoreTo.focus();
			restoreTo = null;
		}
	});

	onMount(() => {
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
		window.addEventListener('keydown', onWindowKeydown);
		return () => window.removeEventListener('keydown', onWindowKeydown);
	});

	function onInputKeydown(e: KeyboardEvent) {
		if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
			e.preventDefault();
			if (matches.length === 0) return;
			const step = e.key === 'ArrowDown' ? 1 : -1;
			highlighted = (highlighted + step + matches.length) % matches.length;
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
				bind:this={input}
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
					<button
						type="button"
						tabindex="-1"
						id={`command-${command.id}`}
						role="option"
						aria-selected={index === highlighted}
						class="option"
						class:highlighted={index === highlighted}
						onpointerenter={() => (highlighted = index)}
						onpointerdown={(e) => e.preventDefault()}
						onclick={() => commands.run(command)}
					>
						<span class="title">{commandTitle(command)}</span>
						{#if hint}<span class="hint">{hint}</span>{/if}
					</button>
				{:else}
					<p class="empty">No matching commands</p>
				{/each}
			</div>
		</div>
	</div>
{/if}

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		z-index: 70;
		background: rgb(var(--ink-rgb) / 0.22);
		backdrop-filter: blur(2px);
		display: flex;
		justify-content: center;
		align-items: flex-start;
		padding: 15vh 16px 16px;
	}
	.palette {
		width: 100%;
		max-width: 520px;
		overflow: hidden;
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: 10px;
		box-shadow: var(--elev-2);
		font: 13px/1.4 var(--font-display);
		color: var(--ink);
	}
	.palette:focus-within {
		border-color: var(--accent);
		box-shadow:
			var(--elev-2),
			0 0 0 3px var(--accent-soft);
	}
	input {
		display: block;
		width: 100%;
		box-sizing: border-box;
		padding: 13px 16px;
		font: inherit;
		font-size: 15px;
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
		padding: 6px;
		max-height: 50vh;
		overflow: auto;
	}
	.option {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 2px;
		width: 100%;
		padding: 8px 10px;
		border: 0;
		border-radius: 6px;
		background: transparent;
		color: inherit;
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.option.highlighted {
		background: var(--accent-soft);
	}
	.empty {
		margin: 0;
		padding: 8px 10px;
		color: var(--text-secondary);
	}
	.hint {
		font-size: 11px;
		color: var(--text-secondary);
	}
</style>
