<script lang="ts">
	import { getAppState, getController } from '$lib/context.svelte';
	import { fmt, signed } from '$lib/format';

	const app = getAppState();
	const ctrl = getController();

	// Fill the track from the Δ=0 baseline to the thumb, so the slider reads as a
	// signed magnitude. The fill's stationary edge sits at zero and marks the
	// commit reference; the moving edge tracks the thumb.
	const sliderSpan = $derived(Math.max(ctrl.sliderMax - ctrl.sliderMin, 1e-9));
	const valPct = $derived(((ctrl.sliderValue - ctrl.sliderMin) / sliderSpan) * 100);
	const zeroPct = $derived(Math.max(0, Math.min(100, ((0 - ctrl.sliderMin) / sliderSpan) * 100)));
	const fillLo = $derived(Math.max(0, Math.min(valPct, zeroPct)));
	const fillHi = $derived(Math.min(100, Math.max(valPct, zeroPct)));
</script>

{#if ctrl.activeSolvable}
	{@const c = ctrl.activeSolvable}
	<div class="slider-block">
		<div class="slider-head mono">
			<span>&Delta; demand</span>
			<span class="val">{signed(ctrl.sliderValue)} MW</span>
		</div>
		<div class="range-mode">
			<div class="segment mono" aria-label="demand range">
				<button
					type="button"
					class:active={app.demandRangeMode === 'local'}
					aria-pressed={app.demandRangeMode === 'local'}
					aria-label="nearby demand range"
					title="range near the selected demand setting"
					onclick={() => ctrl.setDemandRangeMode('local')}>nearby</button
				>
				<button
					type="button"
					class:active={app.demandRangeMode === 'full'}
					aria-pressed={app.demandRangeMode === 'full'}
					aria-label="full demand range"
					title="range from zero load to the local physical limit"
					onclick={() => ctrl.setDemandRangeMode('full')}>full range</button
				>
			</div>
			<span class="mono dim">{fmt.format(ctrl.sliderMin)} to {fmt.format(ctrl.sliderMax)} MW</span>
		</div>
		<input
			type="range"
			min={ctrl.sliderMin}
			max={ctrl.sliderMax}
			step="0.5"
			style="--fill-lo:{fillLo}%; --fill-hi:{fillHi}%"
			bind:value={ctrl.sliderCurrent, ctrl.setSliderPreview}
			aria-label="demand delta at selected bus"
			onpointerdown={() => ctrl.setSliderPreview(ctrl.sliderValue)}
			onkeydown={() => ctrl.setSliderPreview(ctrl.sliderValue)}
			onpointerup={(e) => ctrl.finishDemandInput(Number(e.currentTarget.value))}
			onkeyup={(e) => ctrl.finishDemandInput(Number(e.currentTarget.value))}
			onchange={(e) => ctrl.finishDemandInput(Number(e.currentTarget.value))}
		/>
		<div class="demand-feedback" class:idle={!ctrl.previewing && !ctrl.isPerturbed(c)}>
			<p class="pred mono dim" aria-hidden={!(ctrl.predictedDeltaObj !== null && ctrl.previewing)}>
				{#if ctrl.predictedDeltaObj !== null && ctrl.previewing}
					predicted &Delta;cost {signed(ctrl.predictedDeltaObj)} $/h
				{:else}
					&nbsp;
				{/if}
			</p>
			<p class="score mono" aria-hidden={!(ctrl.gradientScore && ctrl.isPerturbed(c))}>
				{#if ctrl.gradientScore && ctrl.isPerturbed(c)}
					gradient {signed(ctrl.gradientScore.pred)} &middot; exact {signed(
						ctrl.gradientScore.exact
					)}
					$/h
				{:else}
					&nbsp;
				{/if}
			</p>
			<div class="reset-row">
				{#if ctrl.isPerturbed(c)}
					<button class="reset mono" onclick={() => ctrl.resetCase(c)}>reset demand</button>
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
		color: var(--ink-dim);
		margin-bottom: 4px;
	}

	.slider-head .val {
		color: var(--ink);
	}

	.range-mode {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		margin: 6px 0 7px;
		font-size: 10.5px;
	}

	input[type='range'] {
		-webkit-appearance: none;
		appearance: none;
		width: 100%;
		height: 4px;
		background:
			linear-gradient(
				90deg,
				transparent 0 var(--fill-lo, 0%),
				rgb(var(--accent-rgb) / 0.32) var(--fill-lo, 0%) var(--fill-hi, 0%),
				transparent var(--fill-hi, 0%) 100%
			),
			var(--line);
		border-radius: var(--radius-xs);
		outline-offset: 4px;
		margin: 6px 0;
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

	/* Coarse pointers: a larger track and thumb for a comfortable touch target. */
	@media (hover: none), (pointer: coarse) {
		input[type='range'] {
			height: 6px;
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

	.demand-feedback {
		min-height: 78px;
	}

	/* On a fresh selection (no preview, no perturbation) the predicted/gradient/
	   reset rows are empty; collapse the reserved block so the panel isn't padded
	   with whitespace. The reservation returns during interaction to avoid jumps. */
	.demand-feedback.idle {
		min-height: 0;
	}

	.demand-feedback.idle .pred,
	.demand-feedback.idle .score {
		display: none;
	}

	.reset-row {
		min-height: 28px;
	}
</style>
