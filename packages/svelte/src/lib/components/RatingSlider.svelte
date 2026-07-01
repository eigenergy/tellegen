<script lang="ts">
	import { getController } from '../context.svelte.js';
	import { fmt, signed } from '../format.js';

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
	const sliderTip =
		'The black tick marks the last committed rating change. Drag the knob to preview a new thermal rating; release to solve and move the tick to that point.';
	const disabledTip = "this line's rating is synthesized; not adjustable";
	const scoreTip =
		'Gradient is the estimate of total cost change versus base before the solve finishes, from the engine preview at the committed point. Exact is the resolved OPF objective change.';
</script>

{#if ctrl.activeSolvable}
	{@const c = ctrl.activeSolvable}
	{@const branch = ctrl.selectedBranchData}
	<div class="slider-block">
		<div class="slider-head mono">
			<span>&Delta; rating</span>
			<span class="val">{signed(ctrl.ratingSliderValue)} MW</span>
		</div>
		<div class="line-row mono">
			{#if branch}
				<span class="dim"
					>line {branch.from}&#8201;&ndash;&#8201;{branch.to} &middot; {fmt.format(
						branch.rate_mw
					)} MW</span
				>
			{/if}
			<span class="dim">
				{#if bounds.disabled}
					not adjustable
				{:else}
					{fmt.format(bounds.min)} to +{fmt.format(bounds.max)} MW
				{/if}
			</span>
		</div>
		<div
			class="slider-track"
			style="--fill-lo:{fillLo}; --fill-hi:{fillHi}; --neutral-pos:{neutralPos}"
			title={bounds.disabled ? disabledTip : sliderTip}
		>
			<input
				type="range"
				min={bounds.min}
				max={bounds.max}
				step="any"
				disabled={bounds.disabled}
				bind:value={ctrl.ratingSliderCurrent, ctrl.setRatingPreview}
				aria-label="rating delta at selected line"
				onpointerdown={() => ctrl.setRatingPreview(ctrl.ratingSliderValue)}
				onkeydown={() => ctrl.setRatingPreview(ctrl.ratingSliderValue)}
				onpointerup={(e) => ctrl.finishRatingInput(Number(e.currentTarget.value))}
				onkeyup={(e) => ctrl.finishRatingInput(Number(e.currentTarget.value))}
				onchange={(e) => ctrl.finishRatingInput(Number(e.currentTarget.value))}
			/>
		</div>
		<div class="rating-feedback" class:idle={!ctrl.previewing && !ctrl.isPerturbed(c)}>
			<p class="pred mono dim" aria-hidden={!(ctrl.predictedDeltaObj !== null && ctrl.previewing)}>
				{#if ctrl.predictedDeltaObj !== null && ctrl.previewing}
					predicted &Delta;cost {signed(ctrl.predictedDeltaObj)} $/h
				{:else}
					&nbsp;
				{/if}
			</p>
			<p class="score mono" aria-hidden={!(ctrl.gradientScore && ctrl.isPerturbed(c))}>
				{#if ctrl.gradientScore && ctrl.isPerturbed(c)}
					<span title={scoreTip}>
						gradient {signed(ctrl.gradientScore.pred)} &middot; exact {signed(
							ctrl.gradientScore.exact
						)}
						$/h
					</span>
				{:else}
					&nbsp;
				{/if}
			</p>
			<div class="reset-row">
				{#if ctrl.isPerturbed(c)}
					<button class="reset mono" onclick={() => ctrl.resetCase(c)}>reset rating</button>
				{/if}
			</div>
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
		font-size: 11.5px;
		color: var(--text-secondary);
		margin-bottom: 4px;
	}

	.slider-head .val {
		color: var(--ink);
	}

	.line-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		margin: 6px 0 7px;
		font-size: 10.5px;
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

	.pred {
		margin: 2px 0 0;
		font-size: 11px;
		min-height: 16px;
	}

	.score {
		margin: 8px 0 0;
		font-size: 11px;
		color: var(--ink);
		min-height: 16px;
	}

	.rating-feedback {
		min-height: 78px;
	}

	/* On a fresh selection (no preview, no perturbation) the predicted/gradient/
	   reset rows are empty; collapse the reserved block so the panel isn't padded
	   with whitespace. The reservation returns during interaction to avoid jumps. */
	.rating-feedback.idle {
		min-height: 0;
	}

	.rating-feedback.idle .pred,
	.rating-feedback.idle .score {
		display: none;
	}

	.reset-row {
		min-height: 28px;
	}
</style>
