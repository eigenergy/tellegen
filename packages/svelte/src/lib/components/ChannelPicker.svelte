<script lang="ts">
	let {
		channels,
		selected,
		ontoggle,
		onclear
	}: {
		channels: { key: string; label: string; detail: string }[];
		selected: readonly string[];
		ontoggle: (key: string) => void;
		onclear: () => void;
	} = $props();

	let search = $state('');
	const matches = $derived.by(() => {
		const term = search.trim().toLowerCase();
		return channels.filter(({ label, detail, key }) =>
			term ? `${label} ${detail} ${key}`.toLowerCase().includes(term) : selected.includes(key)
		);
	});
</script>

<details>
	<summary>Plot channels ({selected.length})</summary>
	<div class="actions">
		<input
			type="search"
			bind:value={search}
			placeholder="Search equipment or quantity"
			aria-label="Search channels"
		/>
		<button disabled={!selected.length} onclick={onclear}>Clear</button>
	</div>
	<ul>
		{#each matches.slice(0, 32) as channel (channel.key)}
			<li>
				<label>
					<input
						type="checkbox"
						checked={selected.includes(channel.key)}
						onchange={() => ontoggle(channel.key)}
					/>
					{channel.label} <small>{channel.detail}</small>
				</label>
			</li>
		{/each}
	</ul>
	{#if search.trim() && !matches.length}<p>No matching channels</p>{/if}
	{#if matches.length > 32}<p>
			Showing 32 of {matches.length}. Refine the search to see more.
		</p>{/if}
</details>

<style>
	details {
		max-height: 55%;
		overflow: auto;
	}
	summary {
		cursor: pointer;
		padding: 4px 0;
		font-weight: 600;
	}
	.actions,
	label {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.actions input {
		flex: 1;
		min-width: 0;
	}
	ul {
		padding: 0;
		margin: 0;
		list-style: none;
	}
	label {
		padding-block: 3px;
	}
	p,
	small {
		color: var(--text-secondary);
	}
</style>
