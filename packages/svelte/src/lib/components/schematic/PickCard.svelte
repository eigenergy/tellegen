<script lang="ts" generics="T extends { id: string }">
	import type { Snippet } from 'svelte';
	import type { HoverPin } from './view-prefs.svelte.js';

	/** A card beside a point of the diagram listing what is drawn there (branches sharing a route,
	 * buses at one point), so one can be picked. It opens on hover, stays while hovered, and is
	 * pinned by its owner; it flips to the other side of the point near the stage's edge. */
	let {
		x,
		y,
		stage,
		gap,
		reach,
		heading,
		label,
		rows,
		pin,
		selected,
		onhover,
		onpick,
		cells
	}: {
		/** The point, in stage pixels. */
		x: number;
		y: number;
		stage: { width: number; height: number };
		/** Distance from the point to the card. */
		gap: number;
		/** Room the card needs to the right of the point before it flips left. */
		reach: number;
		heading: string;
		label: string;
		rows: T[];
		pin: HoverPin;
		selected: (row: T) => boolean;
		onhover: (row: T | null) => void;
		onpick: (row: T) => void;
		cells: Snippet<[T]>;
	} = $props();

	const right = $derived(x + reach < stage.width);
	const below = $derived(y + 180 < stage.height);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions (Pointer bookkeeping only: the rows are buttons.) -->
<div
	class="card"
	style:left={right ? `${x + gap}px` : null}
	style:right={right ? null : `${stage.width - x + gap}px`}
	style:top={below ? `${y + gap}px` : null}
	style:bottom={below ? null : `${stage.height - y + gap}px`}
	onpointerenter={() => pin.enterCard()}
	onpointerleave={() => pin.leaveCard()}
	onkeydown={(e) => {
		if (e.key === 'Escape') pin.close();
	}}
>
	<p>{heading}{pin.pinned !== null ? '' : ' · click to keep open'}</p>
	<ul aria-label={label}>
		{#each rows as row, i (i)}
			<li>
				<button
					aria-pressed={selected(row)}
					onpointerenter={() => onhover(row)}
					onpointerleave={() => onhover(null)}
					onclick={() => onpick(row)}>{@render cells(row)}</button
				>
			</li>
		{/each}
	</ul>
</div>

<style>
	.card {
		position: absolute;
		z-index: 2;
		width: max-content;
		min-width: 280px;
		max-width: 420px;
		padding: 4px 6px;
		font: 10px var(--font-mono);
		color: var(--text-secondary);
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: 3px;
		box-shadow: var(--elev-1);
	}
	p {
		margin: 0 0 3px;
	}
	ul {
		margin: 0;
		padding: 0;
		list-style: none;
	}
	button {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto auto auto auto;
		gap: 8px;
		width: 100%;
		padding: 2px 4px;
		font: inherit;
		text-align: left;
		color: var(--ink);
		background: none;
		border: 0;
		border-radius: 2px;
		cursor: pointer;
	}
	button:hover,
	button[aria-pressed='true'] {
		background: var(--accent-soft);
	}
	/* Cells come from the owner's snippet. */
	.card :global(.id) {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.card :global(.violation) {
		color: var(--text-danger);
	}
	.card :global(.warning) {
		color: var(--accent);
	}
</style>
