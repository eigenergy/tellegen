<script lang="ts">
	import { busRadius } from '$lib/colors';

	const SIZE_SAMPLES = [10, 100, 500];
</script>

<div class="sizes">
	{#each SIZE_SAMPLES as mw (mw)}
		<span class="size mono">
			<i style:width="{2 * busRadius(mw)}px" style:height="{2 * busRadius(mw)}px"></i>
			{mw}
		</span>
	{/each}
	<span class="mono dim caption">MW, max(load,&#8201;gen)</span>
</div>

<style>
	.sizes {
		display: flex;
		align-items: center;
		gap: 12px;
		font-size: 10px;
		color: var(--ink-dim);
	}

	.size {
		display: inline-flex;
		align-items: center;
		gap: 5px;
	}

	.size i {
		display: inline-block;
		border-radius: 50%;
		background: rgba(212, 116, 34, 0.55);
		border: 1px solid rgba(46, 42, 34, 0.45);
	}

	.caption {
		margin-left: auto;
		font-size: 9.5px;
	}

	@media (max-width: 760px) {
		.sizes {
			flex-wrap: wrap;
			gap: 8px 12px;
		}

		.caption {
			margin-left: 0;
			flex-basis: 100%;
		}
	}
</style>
