<script lang="ts">
	import { search, type SearchIndex, type SearchMatch } from '../diagram/index.js';

	let {
		index,
		onselect,
		hint = () => null,
		placeholder = 'Find a bus or branch'
	}: {
		index: SearchIndex;
		onselect: (match: SearchMatch) => void;
		/** A short result value shown beside each match, such as a voltage or loading. */
		hint?: (match: SearchMatch) => { text: string; state?: string } | null;
		placeholder?: string;
	} = $props();

	const id = $props.id();
	let query = $state('');
	let active = $state(0);
	let open = $state(false);
	const found = $derived(search(index, query, { limit: 10 }));

	function choose(match: SearchMatch | undefined) {
		if (!match) return;
		onselect(match);
		query = '';
		open = false;
	}
	function onkeydown(e: KeyboardEvent) {
		const n = found.matches.length;
		if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
			e.preventDefault();
			if (n) active = (active + (e.key === 'ArrowDown' ? 1 : -1) + n) % n;
			open = true;
		} else if (e.key === 'Enter') {
			e.preventDefault();
			choose(found.matches[active]);
		} else if (e.key === 'Escape') {
			if (query) query = '';
			else open = false;
		}
	}
</script>

<div class="omni">
	<input
		type="search"
		role="combobox"
		aria-label={placeholder}
		aria-expanded={open && !!query.trim()}
		aria-controls="{id}-results"
		aria-activedescendant={open && found.matches.length ? `${id}-${active}` : undefined}
		autocomplete="off"
		spellcheck="false"
		{placeholder}
		bind:value={query}
		oninput={() => {
			active = 0;
			open = true;
		}}
		onfocus={() => (open = true)}
		onblur={() => setTimeout(() => (open = false), 120)}
		{onkeydown}
	/>
	{#if open && query.trim()}
		<ul id="{id}-results" role="listbox" aria-label="Search results">
			{#each found.matches as match, i (`${match.kind}:${match.index}`)}
				{@const h = hint(match)}
				<li
					id="{id}-{i}"
					role="option"
					aria-selected={i === active}
					onpointerdown={(e) => {
						e.preventDefault();
						choose(match);
					}}
				>
					<span class="kind">{match.kind === 'bus' ? 'bus' : match.family}</span>
					<span class="id">{match.id}</span>
					{#if h}<span class="hint {h.state ?? ''}">{h.text}</span>{/if}
				</li>
			{:else}
				<li class="empty" role="presentation">No bus or branch ID contains that text.</li>
			{/each}
			{#if found.total > found.matches.length}<li class="more" role="presentation">
					{found.total.toLocaleString()} matches · keep typing to narrow
				</li>{/if}
		</ul>
	{/if}
</div>

<style>
	.omni {
		position: relative;
	}
	input {
		width: 100%;
		box-sizing: border-box;
		font: 11px var(--font-mono);
		color: var(--ink);
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: 3px;
		padding: 6px 8px;
	}
	ul {
		position: absolute;
		z-index: 5;
		left: 0;
		right: 0;
		top: calc(100% + 2px);
		margin: 0;
		padding: 3px;
		list-style: none;
		max-height: 260px;
		overflow: auto;
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: 3px;
		box-shadow: 0 6px 18px rgba(0, 0, 0, 0.16);
	}
	li {
		display: grid;
		grid-template-columns: 70px 1fr auto;
		gap: 6px;
		align-items: baseline;
		padding: 5px 6px;
		border-radius: 2px;
		cursor: pointer;
	}
	li[aria-selected='true'] {
		background: var(--accent-soft);
	}
	.kind,
	.empty,
	.more {
		color: var(--text-secondary);
	}
	.kind {
		font: 9px var(--font-mono);
		text-transform: uppercase;
	}
	.id,
	.hint {
		font-family: var(--font-mono);
		white-space: nowrap;
	}
	.id {
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.hint.violation {
		color: var(--text-danger);
	}
	.hint.warning {
		color: var(--text-accent);
	}
	.empty,
	.more {
		display: block;
		cursor: default;
	}
</style>
