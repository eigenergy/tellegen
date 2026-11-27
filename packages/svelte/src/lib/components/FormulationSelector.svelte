<script lang="ts">
	import { formulationLabel } from '../format.js';
	import { getController } from '../context.svelte.js';
	import type { SolvableCase } from '../state.svelte.js';
	import { FORMULATIONS, type Formulation } from '@tellegen/engine';

	let {
		currentCase,
		id = 'formulation-select',
		variant = 'select'
	}: {
		/** The case the calculation is for. Defaults to the active case. */
		currentCase?: SolvableCase | null;
		/** The control's id; give each selector on a page its own. */
		id?: string;
		/** `select` is a labelled menu; `segmented` is a compact row of buttons. */
		variant?: 'select' | 'segmented';
	} = $props();

	const ctrl = getController();
	const c = $derived(currentCase === undefined ? ctrl.activeSolvable : currentCase);
	/** The segmented button that holds the tab stop after an arrow key moved it. */
	let focused = $state<Formulation | null>(null);

	function choose(target: SolvableCase, value: Formulation) {
		if (target.solving || value === target.formulation) return;
		if (!ctrl.calculationOffered(target, value)) return;
		void ctrl.chooseCalculation(target, value);
	}

	const STEPS: Record<string, number> = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 };

	/** Arrow keys, Home and End move focus between the offered calculations;
	 * Space, Enter or a click runs one, so moving never starts a solve. */
	function move(event: KeyboardEvent, target: SolvableCase, from: Formulation) {
		const offered = FORMULATIONS.filter((f) => ctrl.calculationOffered(target, f.id));
		if (!offered.length) return;
		const at = offered.findIndex((f) => f.id === from);
		let next: number;
		if (event.key === 'Home') next = 0;
		else if (event.key === 'End') next = offered.length - 1;
		else if (event.key in STEPS)
			next = (Math.max(0, at) + STEPS[event.key] + offered.length) % offered.length;
		else return;
		event.preventDefault();
		focused = offered[next].id;
		const group = (event.currentTarget as HTMLElement).parentElement;
		group?.querySelector<HTMLElement>(`[data-formulation="${focused}"]`)?.focus();
	}

	function leave(event: FocusEvent) {
		const to = event.relatedTarget;
		if (!(to instanceof Node) || !(event.currentTarget as HTMLElement).contains(to)) focused = null;
	}
</script>

{#if c}
	{@const target = c}
	{@const chosen = target.solving
		? (target.settings.calculation ?? target.formulation)
		: target.formulation}
	{@const stop = focused ?? chosen}
	<!-- While a calculation runs, the control names it; otherwise the one on screen. -->
	<div class="formulation">
		{#if variant === 'segmented'}
			<div
				{id}
				class="segmented"
				role="radiogroup"
				aria-label="Calculation"
				aria-busy={target.solving}
				tabindex="-1"
				onfocusout={leave}
			>
				{#each FORMULATIONS as f (f.id)}
					{@const offered = ctrl.calculationOffered(target, f.id)}
					<button
						type="button"
						role="radio"
						data-formulation={f.id}
						aria-checked={f.id === chosen}
						aria-disabled={!offered || target.solving}
						title={f.hint}
						tabindex={f.id === stop ? 0 : -1}
						onclick={() => choose(target, f.id)}
						onkeydown={(event) => move(event, target, f.id)}
					>
						{formulationLabel(
							f.id,
							f.id === target.formulation ? target.solution?.objective_kind : undefined
						)}
					</button>
				{/each}
			</div>
		{:else}
			<label class="formulation-row" for={id}>
				<span>Calculation</span>
				<select
					{id}
					disabled={target.solving}
					value={chosen}
					onchange={(e) => choose(target, e.currentTarget.value as Formulation)}
				>
					{#each FORMULATIONS as f (f.id)}
						<option value={f.id} disabled={!ctrl.calculationOffered(target, f.id)}>
							{formulationLabel(
								f.id,
								f.id === target.formulation ? target.solution?.objective_kind : undefined
							)}
						</option>
					{/each}
				</select>
			</label>
		{/if}
		{#if target.solving}
			<p class="solving" role="status">
				<span>
					{ctrl.acOpfLoading ? 'Loading…' : 'Solving…'}
				</span>
				{#if ctrl.canCancelSettings(target)}
					<button class="cancel" onclick={() => ctrl.cancelSettings(target)}>Cancel</button>
				{/if}
			</p>
		{/if}
	</div>
{/if}

<style>
	.formulation {
		display: grid;
		gap: 6px;
	}

	.formulation-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 10px;
		font-size: var(--fs-body);
		color: var(--ink);
		white-space: nowrap;
	}

	.formulation-row select {
		min-width: 0;
		font-size: var(--fs-body);
	}

	.formulation-row select:disabled {
		opacity: 0.55;
		cursor: progress;
	}

	.segmented {
		display: flex;
		flex-wrap: wrap;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		overflow: hidden;
	}

	.segmented button {
		flex: 1 1 auto;
		padding: 4px 8px;
		font: inherit;
		font-size: var(--fs-label);
		white-space: nowrap;
		color: var(--text-secondary);
		background: var(--surface-control);
		border: 0;
		cursor: pointer;
	}

	.segmented button + button {
		border-left: 1px solid var(--line);
	}

	.segmented button:hover:not([aria-disabled='true']) {
		color: var(--ink);
	}

	.segmented button[aria-checked='true'] {
		background: var(--accent-soft);
		color: var(--text-accent);
	}

	.segmented button[aria-disabled='true'] {
		opacity: 0.55;
		cursor: not-allowed;
	}

	.segmented[aria-busy='true'] button {
		cursor: progress;
	}

	.segmented button:focus-visible {
		outline-offset: -3px;
	}

	.solving {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		margin: 0;
		font-size: var(--fs-body);
		color: var(--text-secondary);
	}

	.cancel {
		font: inherit;
		padding: 2px 8px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		background: var(--surface-control);
		color: var(--ink);
		cursor: pointer;
	}

	.cancel:hover {
		border-color: var(--border-strong);
	}

	@media (hover: none), (pointer: coarse) {
		/* The select is sized to its longest option, which runs off a 320px screen.
		   Let it shrink around the current value rather than truncate the label.
		   16px also keeps Safari from zooming the layout viewport on focus. */
		.formulation-row select {
			flex: 0 1 auto;
			min-width: 0;
			min-height: 44px;
			font-size: 16px;
			padding-block: 6px;
		}

		.formulation-row > span {
			flex: 0 0 auto;
		}

		.segmented button,
		.cancel {
			min-height: 44px;
		}
	}
</style>
