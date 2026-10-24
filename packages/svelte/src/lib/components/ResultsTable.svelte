<script lang="ts">
	import { queryTable, type TableModel } from '../diagram/index.js';

	let {
		model,
		selected,
		onselect,
		hovered = -1,
		onhover
	}: {
		model: TableModel;
		/** Row index (network order) of the current selection, or −1. */
		selected: number;
		onselect: (index: number) => void;
		/** Row under the pointer here or in a linked view (−1 for none). */
		hovered?: number;
		onhover?: (index: number) => void;
	} = $props();

	const ROW = 22;
	const HEIGHT = 240;
	const OVERSCAN = 6;
	let text = $state('');
	let issuesOnly = $state(false);
	let sortKey = $state<string | null>(null);
	let descending = $state(false);
	let scrollTop = $state(0);
	let scroller = $state<HTMLDivElement>();

	const rows = $derived(queryTable(model, { text, issuesOnly, sortKey, descending }));
	const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - OVERSCAN));
	const last = $derived(Math.min(rows.length, first + Math.ceil(HEIGHT / ROW) + 2 * OVERSCAN));
	const visible = $derived(Array.from(rows.subarray(first, last)));
	const template = $derived(
		model.columns
			.map((c) =>
				c.numeric
					? 'minmax(64px, 0.8fr)'
					: c.key === 'id'
						? 'minmax(110px, 1.5fr)'
						: 'minmax(70px, 1fr)'
			)
			.join(' ')
	);
	const format = (v: unknown, digits = 2) =>
		v === null || v === undefined
			? '—'
			: typeof v === 'number'
				? v.toLocaleString(undefined, {
						minimumFractionDigits: digits,
						maximumFractionDigits: digits
					})
				: String(v);

	function sort(key: string, numeric?: boolean) {
		// Numbers start largest-first, text A–Z; a second click reverses.
		descending = sortKey === key ? !descending : !!numeric;
		sortKey = key;
	}
	// Bring the selection into view when it changes elsewhere (map, issues, search).
	$effect(() => {
		const k = rows.indexOf(selected);
		if (k < 0 || !scroller) return;
		const y = k * ROW;
		if (y < scroller.scrollTop || y + ROW > scroller.scrollTop + HEIGHT)
			scroller.scrollTop = y - HEIGHT / 2;
	});
	function onkeydown(e: KeyboardEvent) {
		if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(e.key) || !rows.length) return;
		e.preventDefault();
		const k = rows.indexOf(selected);
		const next =
			e.key === 'Home'
				? 0
				: e.key === 'End'
					? rows.length - 1
					: Math.max(
							0,
							Math.min(rows.length - 1, (k < 0 ? -1 : k) + (e.key === 'ArrowDown' ? 1 : -1))
						);
		onselect(rows[next]);
	}
</script>

<div class="table">
	<div class="toolbar">
		<input
			type="search"
			placeholder="Filter by ID or endpoint"
			aria-label="Filter rows"
			bind:value={text}
		/>
		<label><input type="checkbox" bind:checked={issuesOnly} /> Issues only</label>
	</div>
	<!-- The grid is a keyboard target: arrow keys move the shared selection. -->
	<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
	<div
		class="scroll"
		role="grid"
		aria-label="Results by {model.kind === 'bus' ? 'bus' : 'branch'}"
		aria-rowcount={rows.length}
		tabindex="0"
		style:height="{HEIGHT}px"
		bind:this={scroller}
		onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
		{onkeydown}
	>
		<div class="head" role="row" style:grid-template-columns={template}>
			{#each model.columns as c (c.key)}
				<div
					role="columnheader"
					class:num={c.numeric}
					aria-sort={sortKey === c.key ? (descending ? 'descending' : 'ascending') : 'none'}
				>
					<button title={c.title ?? `Sort by ${c.label}`} onclick={() => sort(c.key, c.numeric)}
						>{c.label}{#if c.unit}<small>{c.unit}</small>{/if}</button
					>
				</div>
			{/each}
		</div>
		<div class="body" style:height="{rows.length * ROW}px">
			{#each visible as i, n (i)}
				{@const row = model.row(i)}
				<!-- Arrow keys on the grid move the selection; a click is the pointer equivalent. -->
				<!-- svelte-ignore a11y_click_events_have_key_events -->
				<div
					class="row {row.state}"
					role="row"
					tabindex="-1"
					aria-selected={i === selected}
					class:hovered={i === hovered}
					style:top="{(first + n) * ROW}px"
					style:grid-template-columns={template}
					onclick={() => onselect(i)}
					onpointerenter={() => onhover?.(i)}
					onpointerleave={() => onhover?.(-1)}
				>
					{#each model.columns as c (c.key)}
						<span role="gridcell" class:num={c.numeric}>{format(row[c.key], c.digits)}</span>
					{/each}
				</div>
			{/each}
		</div>
	</div>
	<p class="count">{rows.length.toLocaleString()} of {model.count.toLocaleString()} rows</p>
</div>

<style>
	.table {
		font-size: 11px;
	}
	.toolbar {
		display: flex;
		gap: 8px;
		align-items: center;
		margin-bottom: 4px;
	}
	.toolbar input[type='search'] {
		flex: 1;
		min-width: 0;
		font: 11px var(--font-mono);
		color: var(--ink);
		background: var(--paper);
		border: 1px solid var(--line);
		border-radius: 3px;
		padding: 5px 7px;
	}
	.toolbar label,
	.count,
	small {
		color: var(--text-secondary);
	}
	.scroll {
		position: relative;
		overflow: auto;
		border: 1px solid var(--line);
		border-radius: 3px;
		font: 10px var(--font-mono);
	}
	.scroll:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: 2px;
	}
	.head {
		position: sticky;
		top: 0;
		z-index: 1;
		display: grid;
		min-width: max-content;
		background: var(--surface-control);
		border-bottom: 1px solid var(--line);
	}
	.head button {
		width: 100%;
		padding: 4px 6px;
		text-align: left;
		font: inherit;
		color: var(--text-secondary);
		background: transparent;
		border: 0;
		cursor: pointer;
		white-space: nowrap;
	}
	.head .num button {
		text-align: right;
	}
	.head [aria-sort='ascending'] button,
	.head [aria-sort='descending'] button {
		color: var(--ink);
	}
	small {
		display: block;
		font-size: 9px;
	}
	.body {
		position: relative;
		min-width: max-content;
	}
	.row {
		position: absolute;
		left: 0;
		right: 0;
		height: 22px;
		display: grid;
		align-items: center;
		cursor: pointer;
		border-bottom: 1px solid color-mix(in srgb, var(--line) 50%, transparent);
	}
	.row span {
		padding: 0 6px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.num {
		text-align: right;
	}
	.row.hovered {
		box-shadow: inset 3px 0 var(--line-strong, var(--text-secondary));
	}
	.row[aria-selected='true'] {
		background: var(--accent-soft);
		box-shadow: inset 3px 0 var(--accent);
	}
	.violation span:last-child {
		color: var(--text-danger);
	}
	.warning span:last-child {
		color: var(--text-accent);
	}
	.inactive,
	.missing,
	.unbounded {
		color: var(--text-secondary);
	}
	.count {
		margin: 4px 0 0;
	}
</style>
