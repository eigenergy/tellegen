<script lang="ts">
	import { onMount } from 'svelte';
	import { getAppState, getController } from '../context.svelte.js';

	const app = getAppState();
	const ctrl = getController();

	const buses = $derived(app.active?.network?.buses ?? app.activeLocal?.view?.buses ?? []);

	let query = $state('');
	let open = $state(false);
	let highlighted = $state(0);
	let inputEl = $state.raw<HTMLInputElement | undefined>(undefined);

	const results = $derived(
		query.trim() === ''
			? []
			: buses.filter((b) => String(b.id).startsWith(query.trim())).slice(0, 30)
	);
	const activeIndex = $derived(Math.max(0, Math.min(highlighted, results.length - 1)));

	function commit(id: number) {
		const caseId = app.activeCaseId;
		const localId = app.activeLocalId;
		if (caseId && app.active) ctrl.selectBus(caseId, id);
		else if (localId && app.activeLocal) ctrl.selectLocalBus(localId, id);
		query = '';
		open = false;
		highlighted = 0;
	}

	function onKeydown(e: KeyboardEvent) {
		if (!open || results.length === 0) return;
		if (e.key === 'ArrowDown') {
			e.preventDefault();
			highlighted = (activeIndex + 1) % results.length;
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			highlighted = (activeIndex - 1 + results.length) % results.length;
		} else if (e.key === 'Enter') {
			e.preventDefault();
			commit(results[activeIndex].id);
		}
	}

	onMount(() => {
		function onWindowKeydown(e: KeyboardEvent) {
			if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.altKey || e.key !== '/') return;
			const target = e.target;
			if (
				target instanceof HTMLElement &&
				(target.isContentEditable || target.closest('input, textarea, select'))
			) {
				return;
			}
			if (!inputEl || buses.length === 0) return;
			e.preventDefault();
			open = true;
			highlighted = 0;
			inputEl.focus();
			inputEl.select();
		}

		window.addEventListener('keydown', onWindowKeydown);
		return () => window.removeEventListener('keydown', onWindowKeydown);
	});
</script>

{#if buses.length > 0}
	<div
		class="bus-lookup"
		title="Press / to focus bus lookup"
		onfocusout={(e) => {
			if (!e.currentTarget.contains(e.relatedTarget as Node | null)) open = false;
		}}
	>
		<label class="mono dim" for="bus-lookup-input">bus lookup</label>
		<input
			id="bus-lookup-input"
			class="mono"
			type="text"
			inputmode="numeric"
			autocomplete="off"
			placeholder="bus id"
			role="combobox"
			aria-keyshortcuts="/"
			aria-expanded={open}
			aria-controls="bus-lookup-listbox"
			aria-activedescendant={open && results.length > 0
				? `bus-lookup-opt-${results[activeIndex].id}`
				: undefined}
			bind:this={inputEl}
			bind:value={query}
			onfocus={() => (open = true)}
			oninput={() => {
				open = true;
				highlighted = 0;
			}}
			onkeydown={onKeydown}
		/>
		{#if open && results.length > 0}
			<ul class="mono" id="bus-lookup-listbox" role="listbox" aria-label="matching buses">
				{#each results as bus, i (bus.id)}
					<li
						id="bus-lookup-opt-{bus.id}"
						role="option"
						aria-selected={i === activeIndex}
						class:active={i === activeIndex}
						onmousedown={(e) => {
							e.preventDefault();
							commit(bus.id);
						}}
					>
						bus {bus.id}
					</li>
				{/each}
			</ul>
		{:else if open && query.trim() !== ''}
			<p class="mono dim empty">no bus matches "{query.trim()}"</p>
		{/if}
	</div>
{/if}

<style>
	.bus-lookup {
		position: absolute;
		right: 72px;
		bottom: 18px;
		z-index: var(--z-chrome);
		width: 246px;
		box-sizing: border-box;
		padding: 7px 10px;
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: var(--radius-sm, 3px);
		backdrop-filter: blur(6px);
		box-shadow: var(--elev-2);
		animation: rise var(--dur-med) var(--ease-out) both;
	}

	label {
		display: block;
		font-size: 10px;
		text-transform: uppercase;
		margin-bottom: 3px;
	}

	input {
		width: 100%;
		padding: 4px 8px;
		font-size: 12.5px;
		background: var(--surface-control);
		border: 1px solid var(--line);
		border-radius: var(--radius-xs, 2px);
		color: var(--ink);
	}

	input:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 1px;
	}

	ul {
		position: absolute;
		z-index: 20;
		top: calc(100% + 2px);
		left: 0;
		right: 0;
		max-height: 180px;
		overflow-y: auto;
		margin: 0;
		padding: 4px 0;
		list-style: none;
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: var(--radius-xs, 2px);
		box-shadow: 0 4px 18px rgba(32, 36, 43, 0.14);
	}

	li {
		padding: 5px 10px;
		font-size: 12px;
		cursor: pointer;
	}

	li.active {
		background: var(--accent-soft);
		color: var(--text-accent);
	}

	.empty {
		position: absolute;
		top: calc(100% + 2px);
		left: 0;
		right: 0;
		padding: 6px 8px;
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: var(--radius-xs, 2px);
		box-shadow: var(--elev-1);
		font-size: 10.5px;
		margin: 4px 0 0;
	}

	@media (max-width: 760px) {
		.bus-lookup {
			top: 106px;
			left: 10px;
			right: 10px;
			bottom: auto;
			width: auto;
			padding: 8px 10px 10px;
		}
	}

	@media (max-width: 420px) {
		.bus-lookup {
			top: 106px;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.bus-lookup {
			animation: none;
		}
	}
</style>
