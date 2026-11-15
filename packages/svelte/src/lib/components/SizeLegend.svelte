<script lang="ts">
	import { busRadius } from '../colors.js';

	const SIZE_SAMPLES = [10, 100, 500];
</script>

<p class="caption">Bus size: the larger of load and generation (MW)</p>
<div class="sizes">
	{#each SIZE_SAMPLES as mw (mw)}
		<span class="size">
			<i style:width="{2 * busRadius(mw)}px" style:height="{2 * busRadius(mw)}px"></i>
			{mw}
		</span>
	{/each}
</div>

<style>
	.sizes {
		display: flex;
		align-items: center;
		gap: 12px;
		font-size: 12px;
		color: var(--text-secondary);
	}

	.size {
		display: inline-flex;
		align-items: center;
		gap: 5px;
	}

	.size i {
		display: inline-block;
		border-radius: 50%;
		background: rgb(var(--ink-rgb) / 0.12);
		border: 1px solid rgb(var(--ink-rgb) / 0.45);
	}

	.caption {
		margin: 0 0 6px;
		font-size: 12px;
		font-weight: 600;
		color: var(--ink);
	}
</style>
