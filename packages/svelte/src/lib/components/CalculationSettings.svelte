<script lang="ts">
	import {
		FORMULATIONS,
		type Formulation,
		describeSettings,
		settingsCatalog,
		type SettingsCatalog,
		type SettingDescriptor
	} from '@tellegen/engine';
	import { getAppState, getController } from '../context.svelte.js';
	import { recordedCalculationSettings } from '../settings.svelte.js';
	import { LocalCase, MulticonductorCase, type SolvableCase } from '../state.svelte.js';
	import McNetworkScope from './McNetworkScope.svelte';
	let { currentCase }: { currentCase?: SolvableCase | MulticonductorCase | null } = $props();
	const app = getAppState();
	const ctrl = getController();
	let catalog = $state<SettingsCatalog>(settingsCatalog);
	let failure = $state('');
	let busy = $state(false);
	let calculation = $state('');
	let draft = $state<Record<string, unknown>>({});
	let revision = $state(0);
	let thermalMode = $state('all');
	let thermalBranches = $state('');
	let lazy = $state(false);
	let maxRounds = $state<number | undefined>();
	let nearBinding = $state<number | undefined>();
	const recorded = $derived(recordedCalculationSettings(app.studyView?.inputJson));
	const current = $derived(
		currentCase === undefined
			? app.studyView
				? null
				: (app.activeMulti ?? ctrl.activeSolvable)
			: currentCase
	);
	const selected = $derived(current ? ctrl.inspectSettings(current) : null);
	const descriptor = $derived(catalog.calculations.find((c) => c.calculation === calculation));
	const fieldOrder = [
		'voltage_envelope',
		'v_low_pu',
		'v_min_pu',
		'v_max_pu',
		'max_iterations',
		'damping',
		'tolerance',
		'zero_voltage_tolerance',
		'absolute_kcl_tolerance',
		'relative_kcl_tolerance'
	];
	const unavailable = $derived(
		current instanceof MulticonductorCase
			? current.mcPfReason
			: descriptor && !canSelect(calculation, descriptor.available)
				? (descriptor.reason ?? 'This calculation is unavailable for this case.')
				: null
	);
	const fields = $derived(
		[...(descriptor?.fields ?? [])].sort(
			(a, b) => fieldOrder.indexOf(a.key) - fieldOrder.indexOf(b.key)
		)
	);
	const unsaved = $derived(
		selected ? calculation !== selected.calculation || !equal(options(), selected.selected) : false
	);
	function equal(a: unknown, b: unknown): boolean {
		if (a && b && typeof a === 'object' && typeof b === 'object') {
			const x = a as Record<string, unknown>,
				y = b as Record<string, unknown>;
			return (
				Object.keys(x).length === Object.keys(y).length &&
				Object.keys(x).every((k) => equal(x[k], y[k]))
			);
		}
		return Object.is(a, b);
	}
	function fill(value: Record<string, unknown>) {
		draft = { ...value };
		thermalMode = typeof value.thermal === 'string' ? value.thermal : 'branches';
		const branches = (value.thermal as { branches?: unknown[] })?.branches;
		thermalBranches = branches?.join(', ') ?? '';
		lazy = value.lazy != null;
		const l = value.lazy as {
			max_rounds?: number;
			near_binding?: number;
		} | null;
		maxRounds = l?.max_rounds;
		nearBinding = l?.near_binding;
	}
	// Refill the draft only when the case or its settings revision changes.
	// Results and re-solves also recompute `selected`, and must not discard
	// edits the user has not applied yet.
	let filledFor = '';
	let filledCase: SolvableCase | MulticonductorCase | null = null;
	$effect(() => {
		const value = selected;
		if (!value) {
			filledFor = '';
			filledCase = null;
			return;
		}
		const key = `${value.case_id}\u0000${value.settings_revision}`;
		if (key !== filledFor || current !== filledCase) {
			filledCase = current;
			filledFor = key;
			calculation = value.calculation;
			revision = value.settings_revision;
			fill(value.selected);
			failure = '';
		}
	});
	let requested = false;
	$effect(() => {
		if (current && !requested) {
			requested = true;
			void describeSettings()
				.then((value) => (catalog = value))
				.catch((error) => (failure = String(error)));
		}
	});
	function canSelect(value: string, available: boolean) {
		if (!current) return false;
		if (current instanceof MulticonductorCase) return value === 'mc_ac_pf' && available;
		if (
			current instanceof LocalCase &&
			current.declaredFormulation &&
			current.declaredFormulation !== value
		)
			return false;
		return value === 'acopf'
			? ctrl.acOpfConfigured && !current.perturbed
			: available && ctrl.formulationAvailable(current, value as Formulation);
	}
	function calculationLabel(value: string) {
		return value === 'mc_ac_pf'
			? 'Distribution AC power flow'
			: (FORMULATIONS.find((f) => f.id === value)?.label ?? value);
	}
	function choose(value: string) {
		calculation = value;
		const d = catalog.calculations.find((c) => c.calculation === value);
		fill(
			current?.settings.resolved[value]?.effective ?? {
				...d?.defaults,
				...current?.settings.baselines[value],
				...current?.settings.options[value]
			}
		);
		failure = '';
	}
	function options(): Record<string, unknown> {
		if (calculation !== 'dcopf') return { ...draft };
		return {
			...draft,
			thermal:
				thermalMode === 'branches'
					? {
							branches: thermalBranches
								.split(',')
								.map((x) => x.trim())
								.filter(Boolean)
								.map((x) => (/^\d+$/.test(x) ? Number(x) : x))
						}
					: thermalMode,
			lazy: lazy
				? {
						...(maxRounds === undefined ? {} : { max_rounds: maxRounds }),
						...(nearBinding === undefined ? {} : { near_binding: nearBinding })
					}
				: null
		};
	}
	function number(field: SettingDescriptor, raw: string) {
		draft = { ...draft, [field.key]: raw === '' ? undefined : Number(raw) };
	}
	async function apply(reset = false) {
		const target = current;
		if (!target) return;
		busy = true;
		failure = '';
		try {
			if (calculation === 'acopf' && !(await ctrl.loadAcOpf()))
				throw new Error('The AC OPF solver could not load in this browser.');
			if (current !== target) return;
			await ctrl.updateSettings(target, calculation, reset ? {} : options(), revision, reset);
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
		} finally {
			busy = false;
		}
	}
	async function run() {
		const target = current;
		if (!target) return;
		busy = true;
		failure = '';
		try {
			if (calculation === 'acopf' && !(await ctrl.loadAcOpf()))
				throw new Error('The AC OPF solver could not load in this browser.');
			if (current !== target) return;
			if (unsaved) await ctrl.updateSettings(target, calculation, options(), revision);
			if (current !== target) return;
			await ctrl.runSettings(target);
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
		} finally {
			busy = false;
		}
	}
</script>

{#snippet setting(field: SettingDescriptor)}
	<label class={{ toggle: field.type === 'boolean' }} title={field.description}>
		<span>{field.label}{field.unit ? ` (${field.unit})` : ''}</span>
		{#if field.type === 'boolean'}
			<input
				type="checkbox"
				checked={draft[field.key] === true}
				onchange={(e) => (draft = { ...draft, [field.key]: e.currentTarget.checked })}
			/>
		{:else if field.type === 'enum'}
			<select
				value={String(draft[field.key])}
				onchange={(e) => (draft = { ...draft, [field.key]: e.currentTarget.value })}
				>{#each field.choices ?? [] as choice (choice)}<option value={choice}>{choice}</option
					>{/each}</select
			>
		{:else}
			<input
				type="number"
				step={field.type === 'integer' ? 1 : 'any'}
				min={field.minimum}
				max={field.maximum}
				value={draft[field.key] as number}
				oninput={(e) => number(field, e.currentTarget.value)}
			/>
		{/if}
	</label>
	<small
		>{field.description} Default: {String(
			((selected?.baseline ?? {}) as Record<string, unknown>)[field.key] ?? field.default
		)}.</small
	>
{/snippet}

{#snippet thermalControls()}
	<label
		>Thermal limits<select bind:value={thermalMode}
			><option value="all">All declared limits</option><option value="rated"
				>Explicit ratings only</option
			><option value="branches">Selected branches</option></select
		></label
	>
	{#if thermalMode === 'branches'}<label
			>Branch IDs or UIDs<input
				type="text"
				bind:value={thermalBranches}
				placeholder="1, 2, branches:3"
			/></label
		>{/if}
	<p>
		Changing angle or thermal enforcement changes the optimization model. Results retain these
		choices.
	</p>
{/snippet}
{#snippet lazyControls()}
	<label class="toggle">Lazy thermal enforcement<input type="checkbox" bind:checked={lazy} /></label
	>
	{#if lazy}<label
			>Maximum enforcement rounds<input
				type="number"
				min="1"
				step="1"
				placeholder="Engine default"
				bind:value={maxRounds}
			/></label
		><label
			>Near-binding fraction<input
				type="number"
				min="0"
				max="1"
				step="any"
				placeholder="Engine default"
				bind:value={nearBinding}
			/></label
		>{/if}
{/snippet}

<div class="calculation-settings" role="region" aria-label="Calculation settings">
	<h3>Calculation</h3>
	{#if current && selected}
		<p>
			Run updates the map and exported case together. The displayed result stays until a calculation
			succeeds.
		</p>
		<label
			>Calculation
			<select
				value={calculation}
				disabled={busy || current.solving}
				onchange={(e) => choose(e.currentTarget.value)}
			>
				{#each catalog.calculations as c (c.calculation)}
					{#if current instanceof MulticonductorCase ? c.calculation === 'mc_ac_pf' : c.calculation !== 'mc_ac_pf' && c.calculation !== 'dcpf'}
						<option value={c.calculation} disabled={!canSelect(c.calculation, c.available)}
							>{calculationLabel(c.calculation)}{!canSelect(c.calculation, c.available)
								? ' (unavailable)'
								: ''}</option
						>
					{/if}
				{/each}
			</select>
		</label>
		<fieldset disabled={busy || current.solving}>
			{#each ['model', 'numerical'] as group (group)}
				{#if fields.some((f) => f.group === group)}
					<h4>{group === 'model' ? 'Model behavior' : 'Numerical solver'}</h4>
					{#each fields.filter((f) => f.group === group && f.type !== 'json' && !f.key.includes('tolerance')) as field (field.key)}
						{@render setting(field)}
					{/each}
					{#if calculation === 'dcopf'}
						{#if group === 'model'}{@render thermalControls()}{:else}{@render lazyControls()}{/if}
					{/if}
					{#if group === 'numerical' && fields.some((f) => f.key.includes('tolerance'))}
						<details>
							<summary>Advanced tolerances</summary>
							{#each fields.filter( (f) => f.key.includes('tolerance') ) as field (field.key)}{@render setting(
									field
								)}{/each}
						</details>
					{/if}
				{/if}
			{/each}
		</fieldset>
		{#if !fields.length}<p>This calculation exposes no additional solver options.</p>{/if}
		{#if current instanceof MulticonductorCase}<McNetworkScope
				scope={current.mcPreparedScope}
				islands={current.mcExcludedIslands}
				preview
			/>{/if}
		{#if unsaved}<p role="status">
				Changes will apply to the next run.
			</p>{:else if selected.pending}<p role="status">
				Selected settings differ from the displayed result. Run to update it.
			</p>{/if}
		{#if unavailable}<p>{unavailable}</p>{/if}
		{#if calculation === 'acopf'}<p>
				The experimental AC OPF solver loads on the first run. It supports the unedited base case;
				prices and case export are unavailable.
			</p>{/if}
		<div class="actions">
			<button disabled={busy || current.solving || !unsaved} onclick={() => void apply()}
				>Apply settings</button
			>
			<button disabled={busy || current.solving} onclick={() => void apply(true)}
				>Restore case defaults</button
			>
			<button
				class="primary"
				disabled={busy || current.solving || !!unavailable}
				onclick={() => void run()}
				>{ctrl.acOpfLoading
					? 'Loading AC OPF solver…'
					: current instanceof MulticonductorCase
						? current.mcSolveLabel
						: 'Run calculation'}</button
			>
			{#if current.solving && ctrl.canCancelSettings(current)}<button
					onclick={() => ctrl.cancelSettings(current)}>Cancel calculation</button
				>{/if}
		</div>
		{#if selected.accepted}<details>
				<summary>Settings used for displayed result ({selected.accepted_calculation})</summary>
				<dl>
					{#each Object.entries(selected.accepted) as [key, value] (key)}<dt>
							{key}
						</dt>
						<dd>{JSON.stringify(value)}</dd>{/each}
				</dl>
			</details>{/if}
	{:else if app.studyView}
		<p>Saved result — calculation settings are read-only.</p>
		{#if recorded}<p>{recorded.calculation}</p>
			<dl>
				{#each Object.entries(recorded.effective) as [key, value] (key)}<dt>{key}</dt>
					<dd>{JSON.stringify(value)}</dd>{/each}
			</dl>{:else}<p>This saved result has no recorded calculation settings.</p>{/if}
	{:else}<p>Select an editable case to configure its calculation.</p>{/if}
	{#if failure}<p role="alert">{failure}</p>{/if}
</div>

<style>
	.calculation-settings {
		padding: var(--tellegen-calculation-padding, 0);
		font-size: 12px;
	}
	h3 {
		margin: 0 0 10px;
	}
	h4 {
		margin: 14px 0 8px;
	}
	p,
	small {
		line-height: 1.5;
	}
	small {
		font-size: inherit;
		display: block;
		opacity: 0.75;
		margin-bottom: 10px;
	}
	fieldset {
		border: 0;
		padding: 0;
		margin: 0;
		min-width: 0;
	}
	label {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		margin: 10px 0 4px;
	}
	input[type='number'] {
		width: 105px;
	}
	input[type='text'] {
		width: 160px;
	}
	select,
	input:not([type='checkbox']) {
		padding: 4px 6px;
		border: 1px solid var(--border-strong);
		border-radius: 4px;
		background: var(--surface-control);
		color: var(--ink);
		font: inherit;
	}
	select,
	input {
		max-width: 55%;
	}
	button {
		min-height: 28px;
		padding: 6px 10px;
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-sm, 3px);
		background: var(--paper);
		color: var(--ink);
		font: inherit;
		line-height: 1.25;
		cursor: pointer;
	}
	button:not(.primary):hover:not(:disabled) {
		border-color: var(--accent);
		background: var(--accent-soft);
	}
	button.primary {
		border-color: var(--accent);
		background: var(--accent);
		color: var(--on-accent, var(--paper));
		font-weight: 600;
	}
	button:disabled {
		cursor: default;
		opacity: 0.6;
	}
	button:focus-visible,
	select:focus-visible,
	input:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
	}
	@media (pointer: coarse) {
		button,
		select,
		input:not([type='checkbox']) {
			min-height: 44px;
		}
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		margin: 16px 0;
	}
	dt {
		margin-top: 6px;
	}
	dd {
		margin: 0;
		overflow-wrap: anywhere;
	}
</style>
