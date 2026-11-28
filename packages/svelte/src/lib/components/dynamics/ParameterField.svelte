<script lang="ts">
	/** One parameter of a model or a block: its name and unit, the value as
	 * typed, a hint of what it does, and why the text is not a value when it
	 * is not one. The unit editor (DYR parameters) and the block-diagram
	 * editor (block parameters) show their parameters with it, so both label
	 * and check them alike.
	 * @block Parameter field */
	import { numberError } from '../../dynamics/live-logic.js';

	let {
		name,
		unit = '',
		description = '',
		text,
		min,
		changed = false,
		key,
		oninput
	}: {
		name: string;
		unit?: string;
		description?: string;
		/** What the field shows: the value, or the text being typed. */
		text: string;
		/** The smallest value accepted. */
		min?: number;
		/** Differs from the value in effect (drawn highlighted). */
		changed?: boolean;
		/** Names the input (`data-param`), so focus can return to it. */
		key?: string;
		oninput: (text: string) => void;
	} = $props();

	const error = $derived(numberError(text, min));
	const id = $props.id();
</script>

<label class="pf" class:changed class:invalid={!!error} title={description}
	><span class="pname">{name}{#if unit}<span class="unit-of">{unit}</span>{/if}</span>
	<input
		class="mono"
		inputmode="decimal"
		spellcheck="false"
		value={text}
		data-param={key}
		aria-invalid={error ? 'true' : undefined}
		aria-describedby={error ? `${id}-err` : undefined}
		oninput={(e) => oninput(e.currentTarget.value)}
	/>{#if error}<span class="err" id={`${id}-err`} role="alert">{error}</span>{/if}</label
>

<style>
	.pf {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 9ch;
		align-items: center;
		gap: 2px 6px;
		font-size: var(--fs-control);
	}
	.pname {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--text-secondary);
	}
	.unit-of {
		margin-left: 3px;
		color: var(--text-tertiary);
		font-size: var(--fs-meta);
	}
	input {
		width: 100%;
		min-height: 22px;
		padding: 1px 5px;
		text-align: right;
	}
	.changed .pname {
		color: var(--text-accent);
	}
	.changed input {
		border-color: var(--accent);
		background: var(--accent-soft);
	}
	.invalid input {
		border-color: var(--text-danger);
	}
	.err {
		grid-column: 1 / -1;
		color: var(--text-danger);
		font-size: var(--fs-meta);
	}
</style>
