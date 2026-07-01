<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';

	const app = getAppState();
	const ctrl = getController();

	const buses = $derived(app.active?.network?.buses ?? app.activeLocal?.view?.buses ?? []);

	let query = $state('');
	let open = $state(false);
	let highlighted = $state(0);

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
</script>

{#if buses.length > 0}
	<div
		class="picker"
		onfocusout={(e) => {
			if (!e.currentTarget.contains(e.relatedTarget as Node | null)) open = false;
		}}
	>
		<label class="mono dim" for="bus-picker-input">jump to bus</label>
		<input
			id="bus-picker-input"
			class="mono"
			type="text"
			inputmode="numeric"
			autocomplete="off"
			placeholder="bus id&hellip;"
			role="combobox"
			aria-expanded={open}
			aria-controls="bus-picker-listbox"
			aria-activedescendant={open && results.length > 0
				? `bus-picker-opt-${results[activeIndex].id}`
				: undefined}
			bind:value={query}
			onfocus={() => (open = true)}
			oninput={() => {
				open = true;
				highlighted = 0;
			}}
			onkeydown={onKeydown}
		/>
		{#if open && results.length > 0}
			<ul class="mono" id="bus-picker-listbox" role="listbox" aria-label="matching buses">
				{#each results as bus, i (bus.id)}
					<li
						id="bus-picker-opt-{bus.id}"
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
	.picker {
		position: relative;
		margin-bottom: 10px;
	}

	label {
		display: block;
		font-size: 10px;
		text-transform: uppercase;
		margin-bottom: 4px;
	}

	input {
		width: 100%;
		padding: 5px 8px;
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
		font-size: 10.5px;
		margin: 4px 0 0;
	}
</style>
