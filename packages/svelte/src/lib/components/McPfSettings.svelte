<script lang="ts">
	import { DEFAULT_MC_PF_OPTIONS, validateMcPfOptions, type McPfOptions } from '@tellegen/engine';
	let {
		options = $bindable<McPfOptions>({ ...DEFAULT_MC_PF_OPTIONS }),
		disabled = false,
		onapply
	}: {
		options: McPfOptions;
		disabled?: boolean;
		onapply?: (options: McPfOptions) => Promise<unknown>;
	} = $props();
	let failure = $state('');
	const validation = $derived(validateMcPfOptions(options));
	// Keep one shared set of next-run settings for every solve action. Fill
	// omitted options when opening an older snapshot with partial settings.
	$effect(() => {
		if (Object.keys(DEFAULT_MC_PF_OPTIONS).some((key) => !(key in options))) {
			options = { ...DEFAULT_MC_PF_OPTIONS, ...options };
		}
	});
	async function apply() {
		failure = '';
		if (validation) return;
		try {
			await onapply?.({ ...options });
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
		}
	}
</script>

<details class="settings">
	<summary>Power flow settings</summary>
	<p>
		These settings apply to both Run power flow and Solve energized network. Editing them does not
		start a calculation. A failed run retains the previous result and the settings used for that
		result.
	</p>
	<fieldset {disabled}>
		<legend>Load-model envelope</legend>
		<label class="toggle"
			><input type="checkbox" bind:checked={options.voltage_envelope} />Use load-model envelope</label
		>
		<p>
			Outside the normal band, the envelope changes the load equations to help convergence. Modified
			results are labelled explicitly. These thresholds are not operating voltage limits.
		</p>
		<label>Vlow (pu)<input type="number" step="any" bind:value={options.v_low_pu} /></label>
		<label>Vmin (pu)<input type="number" step="any" bind:value={options.v_min_pu} /></label>
		<label>Vmax (pu)<input type="number" step="any" bind:value={options.v_max_pu} /></label>
		<p>
			With the envelope disabled, the original load equations are used at every voltage; convergence
			is not guaranteed.
		</p>
	</fieldset>
	<fieldset {disabled}>
		<legend>Numerical solver</legend>
		<label
			>Maximum iterations<input
				type="number"
				min="1"
				step="1"
				bind:value={options.max_iterations}
			/></label
		>
		<label
			>Damping<input type="number" min="0" max="1" step="any" bind:value={options.damping} /></label
		>
		<details>
			<summary>Advanced tolerances</summary>
			<label
				>Voltage-change tolerance (V)<input
					type="number"
					step="any"
					bind:value={options.tolerance}
				/></label
			>
			<label
				>Zero-voltage tolerance (V)<input
					type="number"
					step="any"
					bind:value={options.zero_voltage_tolerance}
				/></label
			>
			<label
				>Absolute KCL tolerance (A)<input
					type="number"
					step="any"
					bind:value={options.absolute_kcl_tolerance}
				/></label
			>
			<label
				>Relative KCL tolerance<input
					type="number"
					step="any"
					bind:value={options.relative_kcl_tolerance}
				/></label
			>
		</details>
	</fieldset>
	{#if validation}<p role="alert">{validation}</p>{/if}
	{#if failure}<p role="alert">{failure}</p>{/if}
	<div class="actions">
		{#if onapply}<button disabled={disabled || !!validation} onclick={() => void apply()}
				>Apply settings and run</button
			>{/if}
		<button
			{disabled}
			onclick={() => {
				options = { ...DEFAULT_MC_PF_OPTIONS };
				failure = '';
			}}>Restore defaults</button
		>
	</div>
</details>

<style>
	.settings {
		margin: 0.75rem 0;
		font-size: 12px;
	}
	summary {
		cursor: pointer;
		font-weight: 600;
	}
	fieldset {
		margin: 0.75rem 0;
		border: 1px solid var(--line);
		border-radius: 4px;
	}
	label {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
		margin: 0.5rem 0;
	}
	input[type='number'] {
		width: 7rem;
		min-width: 0;
	}
	.toggle {
		justify-content: flex-start;
	}
	p {
		line-height: 1.5;
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.5rem;
	}
	[role='alert'] {
		color: var(--text-primary);
		border-left: 3px solid #b7791f;
		padding-left: 0.5rem;
	}
</style>
