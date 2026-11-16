<script lang="ts">
	import { getController } from '../context.svelte.js';
	import { signed } from '../format.js';
	import { sliderKeyValue } from '../slider.js';

	const ctrl = getController();

	// Fill from the committed rating to the live thumb. The number line stays
	// stable after release; only the committed tick moves.
	const thumbSizePx = 14;
	const bounds = $derived(ctrl.ratingBounds);
	const sliderSpan = $derived(Math.max(bounds.max - bounds.min, 1e-9));
	const valFrac = $derived(
		Math.max(0, Math.min(1, (ctrl.ratingSliderValue - bounds.min) / sliderSpan))
	);
	const neutralFrac = $derived(
		Math.max(0, Math.min(1, (ctrl.committedRating - bounds.min) / sliderSpan))
	);
	const thumbAlignedPos = (frac: number) =>
		`calc(${frac * 100}% + ${(0.5 - frac) * thumbSizePx}px)`;
	const valPos = $derived(thumbAlignedPos(valFrac));
	const neutralPos = $derived(thumbAlignedPos(neutralFrac));
	const fillLo = $derived(valFrac < neutralFrac ? valPos : neutralPos);
	const fillHi = $derived(valFrac < neutralFrac ? neutralPos : valPos);
</script>

{#if ctrl.activeSolvable}
	{@const c = ctrl.activeSolvable}
	<div class="slider-block">
		<div class="slider-head">
			<span>Rating change</span>
			<span class="val mono">{signed(ctrl.ratingSliderValue)} MW</span>
		</div>
		<div
			class="slider-track"
			style="--fill-lo:{fillLo}; --fill-hi:{fillHi}; --neutral-pos:{neutralPos}"
			title={bounds.disabled ? 'Fixed' : undefined}
		>
			<input
				type="range"
				min={bounds.min}
				max={bounds.max}
				step="any"
				disabled={bounds.disabled}
				bind:value={ctrl.ratingSliderCurrent, ctrl.setRatingPreview}
				aria-label="Rating delta at selected line"
				onpointerdown={() => ctrl.setRatingPreview(ctrl.ratingSliderValue)}
				onkeydown={(e) => {
					const next = sliderKeyValue(e.key, ctrl.ratingSliderValue, bounds.min, bounds.max);
					if (next === null) {
						ctrl.setRatingPreview(ctrl.ratingSliderValue);
						return;
					}
					e.preventDefault();
					ctrl.setRatingPreview(next);
				}}
				onpointerup={(e) => ctrl.finishRatingInput(Number(e.currentTarget.value))}
				onkeyup={(e) => ctrl.finishRatingInput(Number(e.currentTarget.value))}
				onchange={(e) => ctrl.finishRatingInput(Number(e.currentTarget.value))}
			/>
		</div>
		<!-- The slider's ends, as ticks under the track. -->
		<div class="ticks mono" aria-hidden={!bounds.disabled}>
			{#if bounds.disabled}
				<span class="dim">Not adjustable</span>
			{:else}
				<span>{signed(bounds.min)}</span><span>{signed(bounds.max)} MW</span>
			{/if}
		</div>
		<div class="rating-feedback">
			{#if ctrl.predictedDeltaObj !== null && ctrl.previewing}
				<p class="pred dim">
					Predicted <span class="mono">{signed(ctrl.predictedDeltaObj)}</span> $/h
				</p>
			{:else if ctrl.gradientScore && ctrl.isPerturbed(c)}
				<p class="score">
					<span>Predicted <span class="mono">{signed(ctrl.gradientScore.pred)}</span> $/h</span>
					<span>Solved <span class="mono">{signed(ctrl.gradientScore.exact)}</span> $/h</span>
				</p>
			{/if}
			{#if ctrl.committedRating !== 0}
				<button class="reset" onclick={() => ctrl.resetRating(c)}>Reset rating</button>
			{/if}
		</div>
	</div>
{/if}

<style>
	.slider-block {
		margin-top: 14px;
	}

	.slider-head {
		display: flex;
		justify-content: space-between;
		font-size: 13px;
		color: var(--text-secondary);
		margin-bottom: 4px;
	}

	.slider-head .val {
		color: var(--ink);
	}

	.ticks {
		display: flex;
		justify-content: space-between;
		gap: 8px;
		margin: -2px 0 6px;
		font-size: var(--fs-label);
		font-variant-numeric: tabular-nums;
		color: var(--text-secondary);
	}

	.slider-track {
		position: relative;
		margin: 6px 0;
		isolation: isolate;
	}

	.slider-track::before {
		content: '';
		position: absolute;
		top: 50%;
		left: var(--neutral-pos, 50%);
		z-index: 0;
		width: 2px;
		height: 16px;
		background: var(--ink);
		border-radius: 1px;
		box-shadow: 0 0 0 1px rgb(var(--paper-rgb) / 0.72);
		transform: translate(-50%, -50%);
		pointer-events: none;
	}

	input[type='range'] {
		-webkit-appearance: none;
		appearance: none;
		position: relative;
		z-index: 1;
		width: 100%;
		height: 4px;
		padding: 7px 0;
		box-sizing: content-box;
		background:
			linear-gradient(
				90deg,
				transparent 0 var(--fill-lo, 0%),
				rgb(var(--accent-rgb) / 0.32) var(--fill-lo, 0%) var(--fill-hi, 0%),
				transparent var(--fill-hi, 0%) 100%
			),
			var(--line);
		background-clip: content-box;
		border-radius: var(--radius-xs);
		outline-offset: 4px;
		margin: 0;
	}

	input[type='range']:disabled {
		opacity: 0.45;
	}

	input[type='range']::-webkit-slider-thumb {
		-webkit-appearance: none;
		appearance: none;
		width: 14px;
		height: 14px;
		border-radius: 50%;
		background: var(--accent);
		border: 2px solid var(--paper);
		box-shadow: var(--shadow-thumb);
		cursor: ew-resize;
		transition:
			transform var(--dur-fast) var(--ease-out),
			box-shadow var(--dur-fast) var(--ease-out);
	}

	input[type='range']::-moz-range-thumb {
		width: 14px;
		height: 14px;
		border-radius: 50%;
		background: var(--accent);
		border: 2px solid var(--paper);
		box-shadow: var(--shadow-thumb);
		cursor: ew-resize;
		transition:
			transform var(--dur-fast) var(--ease-out),
			box-shadow var(--dur-fast) var(--ease-out);
	}

	input[type='range']:active::-webkit-slider-thumb,
	input[type='range']:focus-visible::-webkit-slider-thumb {
		transform: scale(1.15);
		box-shadow: 0 2px 7px rgb(var(--ink-rgb) / 0.35);
	}

	input[type='range']:active::-moz-range-thumb,
	input[type='range']:focus-visible::-moz-range-thumb {
		transform: scale(1.15);
		box-shadow: 0 2px 7px rgb(var(--ink-rgb) / 0.35);
	}

	/* Coarse pointers: a larger track and thumb for a comfortable touch target.
	   The input's own border box grows to a 44px tap target (padding is symmetric,
	   so the thumb -- centered on the input's box by default -- stays centered on
	   the thinner painted line); background-clip keeps the visible line itself thin. */
	@media (hover: none), (pointer: coarse) {
		input[type='range'] {
			height: 6px;
			padding: 19px 0;
			box-sizing: content-box;
			background-clip: content-box;
		}

		input[type='range']::-webkit-slider-thumb {
			width: 20px;
			height: 20px;
		}

		input[type='range']::-moz-range-thumb {
			width: 20px;
			height: 20px;
		}
	}

	.pred,
	.score {
		margin: 2px 0 0;
		font-size: 13px;
	}

	.score {
		display: flex;
		gap: 16px;
		color: var(--ink);
	}

	.rating-feedback {
		display: grid;
		justify-items: start;
		gap: 6px;
	}
</style>
