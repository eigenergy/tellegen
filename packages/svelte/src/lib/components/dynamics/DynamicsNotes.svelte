<script lang="ts">
	import { COLLAPSE_AFTER, groupNotes } from '../../dynamics/notes.js';

	let { notes, events = [] }: { notes: readonly string[]; events?: readonly string[] } = $props();
	const groups = $derived(groupNotes(notes));
</script>

<div class="notes">
	{#each groups as g (g.id)}
		<details open={g.notes.length <= COLLAPSE_AFTER}>
			<summary>{g.title} <span class="n">{g.notes.length}</span></summary>
			<ul>
				{#each g.notes as n, i (i)}<li>{n}</li>{/each}
			</ul>
		</details>
	{/each}
	{#if events.length}
		<details open={events.length <= COLLAPSE_AFTER}>
			<summary>Run events <span class="n">{events.length}</span></summary>
			<ul>
				{#each events as n, i (i)}<li class="mono">{n}</li>{/each}
			</ul>
		</details>
	{/if}
</div>

<style>
	.notes {
		display: grid;
		gap: 4px;
		font-size: var(--fs-control);
	}
	summary {
		cursor: pointer;
		color: var(--text-secondary);
	}
	.n {
		color: var(--text-tertiary);
		font-variant-numeric: tabular-nums;
	}
	ul {
		margin: 4px 0;
		padding-left: 16px;
		line-height: 1.4;
	}
	li {
		margin: 2px 0;
		overflow-wrap: anywhere;
	}
	.mono {
		font-family: var(--font-mono);
		font-size: var(--fs-meta);
	}
</style>
