<script lang="ts" module>
	import { DC_VARIANTS } from '../dc-variants.js';

	/** Whether a transmission calculation can be chosen. AC OPF loads when it is
	 * chosen here, so the engine reporting it unavailable before the first load
	 * does not hide it while the controller offers it. */
	export function transmissionSelectable(
		value: string,
		available: boolean,
		offered: boolean
	): boolean {
		return (value === 'acopf' || available) && offered;
	}

	/** Plain names for the engine's settings, in place of its own labels. */
	const LABELS: Record<string, string> = {
		angle_difference: 'Angle limits',
		branch_susceptance_formula: 'DC line model',
		start: 'Starting voltages',
		voltage_envelope: 'Load envelope',
		v_low_pu: 'Low voltage',
		v_min_pu: 'Min voltage',
		v_max_pu: 'Max voltage',
		max_iterations: 'Max iterations',
		damping: 'Damping',
		tolerance: 'Voltage tolerance',
		zero_voltage_tolerance: 'Zero-voltage floor',
		absolute_kcl_tolerance: 'Current tolerance',
		relative_kcl_tolerance: 'Relative tolerance',
		thermal: 'Apply to',
		lazy: 'Add line limits as needed'
	};

	/** Plain names for enum choices; null is the case's own choice. */
	const CHOICES: Record<string, string> = {
		flat: 'Flat',
		case: 'From case',
		all: 'All lines',
		rated: 'Rated lines',
		...Object.fromEntries(DC_VARIANTS.map((v) => [v.id, v.label]))
	};

	/** A choice's tooltip: the tool whose convention a DC line model matches. */
	const CHOICE_TITLES: Record<string, string> = Object.fromEntries(
		DC_VARIANTS.filter((v) => v.matches).map((v) => [v.id, `Matches ${v.matches}`])
	);

	/** A setting's value in words. */
	function valueWords(value: unknown): string {
		if (value === null || value === undefined) return 'Default';
		if (value === true) return 'On';
		if (value === false) return 'Off';
		if (typeof value === 'string') return CHOICES[value] ?? value.replaceAll('_', ' ');
		if (typeof value === 'number')
			return value.toLocaleString(undefined, { maximumFractionDigits: 6 });
		if (typeof value === 'object' && value && 'branches' in value) return 'Chosen lines';
		if (typeof value === 'object' && value) return 'On';
		return String(value);
	}

	/** Where each setting sits: settings are grouped by topic, not by calculation. */
	const MODEL = [
		'branch_susceptance_formula',
		'angle_difference',
		'voltage_envelope',
		'v_low_pu',
		'v_min_pu',
		'v_max_pu'
	];
	const SOLVER = ['start', 'max_iterations', 'damping'];
	const TOLERANCES = [
		'tolerance',
		'zero_voltage_tolerance',
		'absolute_kcl_tolerance',
		'relative_kcl_tolerance'
	];
</script>

<script lang="ts">
	import {
		type Formulation,
		describeSettings,
		settingsCatalog,
		type SettingsCatalog,
		type SettingDescriptor
	} from '@tellegen/engine';
	import { getAppState, getController } from '../context.svelte.js';
	import { formulationLabel } from '../format.js';
	import { recordedCalculationSettings } from '../settings.svelte.js';
	import { MulticonductorCase, type SolvableCase } from '../state.svelte.js';

	/** The calculation section of Settings: the calculation and its options as label and control
	 * rows. A change applies and reruns at once. */
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
	const fields = $derived(
		(descriptor?.fields ?? []).filter((f) => f.type !== 'json') as SettingDescriptor[]
	);
	const byKey = (keys: string[]) => keys.flatMap((key) => fields.filter((f) => f.key === key));
	const unavailable = $derived(
		current instanceof MulticonductorCase
			? !!current.mcPfReason
			: !!descriptor && !canSelect(calculation, descriptor.available)
	);
	const solving = $derived(!!current?.solving);
	/** The load envelope's band must be ordered. */
	const bandError = $derived.by(() => {
		const [low, min, max] = ['v_low_pu', 'v_min_pu', 'v_max_pu'].map((k) => draft[k]);
		if ([low, min, max].some((v) => typeof v !== 'number')) return '';
		return 0 < (low as number) &&
			(low as number) < (min as number) &&
			(min as number) < (max as number)
			? ''
			: 'Need low < min < max voltage';
	});

	function fill(value: Record<string, unknown>) {
		draft = { ...value };
		thermalMode = typeof value.thermal === 'string' ? value.thermal : 'branches';
		const branches = (value.thermal as { branches?: unknown[] })?.branches;
		thermalBranches = branches?.join(', ') ?? '';
		lazy = value.lazy != null;
		const l = value.lazy as { max_rounds?: number; near_binding?: number } | null;
		maxRounds = l?.max_rounds;
		nearBinding = l?.near_binding;
	}
	// Refill the draft only when the case or its settings revision changes, so a
	// result arriving does not discard a value being typed.
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
		return transmissionSelectable(
			value,
			available,
			ctrl.calculationOffered(current, value as Formulation)
		);
	}
	function calculationLabel(value: string) {
		return value === 'mc_ac_pf'
			? 'Distribution AC power flow'
			: formulationLabel(value as Formulation);
	}
	function fieldLabel(key: string): string {
		return LABELS[key] ?? key.replaceAll('_', ' ');
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
	/** Apply the form and rerun: the one way a change takes effect. */
	async function apply(reset = false) {
		const target = current;
		if (!target || (!reset && bandError)) return;
		busy = true;
		failure = '';
		try {
			if (calculation === 'acopf' && !(await ctrl.loadAcOpf()))
				throw new Error('AC OPF unavailable');
			if (current !== target) return;
			await ctrl.updateSettings(target, calculation, reset ? {} : options(), revision, reset);
			if (current !== target) return;
			await ctrl.runSettings(target);
		} catch (error) {
			failure = error instanceof Error ? error.message : String(error);
		} finally {
			busy = false;
		}
	}
	/** Restore the case's own settings and rerun. */
	export function restoreDefaults() {
		if (current && !current.solving) void apply(true);
	}
	function set(key: string, value: unknown) {
		draft = { ...draft, [key]: value };
		void apply();
	}
	/** The failure in a few words; the full message is in Details. */
	const failureWords = $derived(
		/infeasib/i.test(failure)
			? 'Infeasible'
			: /converge/i.test(failure)
				? 'Did not converge'
				: /changed/i.test(failure)
					? 'Settings changed; run again'
					: `${calculationLabel(calculation)} failed`
	);
</script>

{#snippet control(field: SettingDescriptor)}
	{@const id = `setting-${field.key}`}
	<div class="row">
		<label for={id}>{fieldLabel(field.key)}{field.unit ? ` (${field.unit})` : ''}</label>
		{#if field.type === 'boolean'}
			<input
				{id}
				type="checkbox"
				class="switch"
				checked={draft[field.key] === true}
				onchange={(e) => set(field.key, e.currentTarget.checked)}
			/>
		{:else if field.type === 'enum'}
			<select
				{id}
				value={String(draft[field.key] ?? null)}
				onchange={(e) =>
					set(
						field.key,
						(field.choices ?? []).find((choice) => String(choice) === e.currentTarget.value)
					)}
				>{#each field.choices ?? [] as choice (choice)}<option
						value={String(choice)}
						title={CHOICE_TITLES[String(choice)]}>{valueWords(choice)}</option
					>{/each}</select
			>
		{:else}
			<input
				{id}
				type="number"
				step={field.type === 'integer' ? 1 : 'any'}
				min={field.minimum}
				max={field.maximum}
				value={draft[field.key] as number}
				onchange={(e) =>
					set(field.key, e.currentTarget.value === '' ? undefined : Number(e.currentTarget.value))}
			/>
		{/if}
	</div>
{/snippet}

<div class="calculation-settings" role="region" aria-label="Calculation settings">
	{#if current && selected}
		<!-- The calculation is chosen in the Network panel; these are its options, by topic. -->
		{#if byKey(MODEL).length}<h3>Model</h3>{/if}
		<fieldset disabled={busy || solving}>
			{#each byKey(MODEL) as field (field.key)}{@render control(field)}{/each}
			{#if bandError}<p class="error" role="alert">{bandError}</p>{/if}
		</fieldset>

		{#if calculation === 'dcopf'}
			<h3>Line limits</h3>
			<fieldset disabled={busy || solving}>
				<div class="row">
					<label for="setting-thermal">Apply to</label>
					<select
						id="setting-thermal"
						bind:value={thermalMode}
						onchange={() => thermalMode !== 'branches' && void apply()}
						><option value="all">All lines</option><option value="rated">Rated lines</option><option
							value="branches">Chosen lines</option
						></select
					>
				</div>
				{#if thermalMode === 'branches'}
					<div class="row">
						<label for="setting-branches">Lines</label>
						<input
							id="setting-branches"
							type="text"
							bind:value={thermalBranches}
							placeholder="1, 2, 3"
							onchange={() => void apply()}
						/>
					</div>
				{/if}
				<div class="row">
					<label for="setting-lazy">Add line limits as needed</label>
					<input
						id="setting-lazy"
						type="checkbox"
						class="switch"
						bind:checked={lazy}
						onchange={() => void apply()}
					/>
				</div>
				{#if lazy}
					<div class="row">
						<label for="setting-rounds">Max rounds</label>
						<input
							id="setting-rounds"
							type="number"
							min="1"
							step="1"
							placeholder="20"
							bind:value={maxRounds}
							onchange={() => void apply()}
						/>
					</div>
					<div class="row">
						<label for="setting-near">Add above (%)</label>
						<input
							id="setting-near"
							type="number"
							min="0"
							max="100"
							step="any"
							placeholder="98"
							value={nearBinding === undefined ? '' : +(nearBinding * 100).toFixed(4)}
							onchange={(e) => {
								const raw = e.currentTarget.value;
								nearBinding = raw === '' ? undefined : Number(raw) / 100;
								void apply();
							}}
						/>
					</div>
				{/if}
			</fieldset>
		{/if}

		{#if byKey(SOLVER).length || byKey(TOLERANCES).length}
			<h3>Solver</h3>
			<fieldset disabled={busy || solving}>
				{#each byKey(SOLVER) as field (field.key)}{@render control(field)}{/each}
				{#if byKey(TOLERANCES).length}
					<details>
						<summary>Tolerances</summary>
						{#each byKey(TOLERANCES) as field (field.key)}{@render control(field)}{/each}
					</details>
				{/if}
			</fieldset>
		{/if}

		<div class="status" role="status">
			{#if solving}
				<span>{ctrl.acOpfLoading ? 'Loading…' : 'Solving…'}</span>
				{#if ctrl.canCancelSettings(current)}<button
						class="text"
						onclick={() => ctrl.cancelSettings(current)}>Cancel</button
					>{/if}
			{:else if failure}
				<span class="error" role="alert" title={failure}>{failureWords}</span>
			{:else if selected.pending && !unavailable}
				<span class="chip">Not applied</span>
				<button class="text" onclick={() => void apply()}>Run</button>
			{:else if unavailable}
				<span>Unavailable for this case</span>
			{/if}
		</div>
		{#if failure}
			<details class="failure">
				<summary>Details</summary>
				<p>{failure}</p>
			</details>
		{/if}
	{:else if app.studyView}
		<div class="row"><span>Saved result</span><span class="value">Read-only</span></div>
		{#if recorded}
			<dl>
				<div class="row">
					<dt>Calculation</dt>
					<dd>{calculationLabel(recorded.calculation)}</dd>
				</div>
				{#each Object.entries(recorded.effective) as [key, value] (key)}
					<div class="row">
						<dt>{fieldLabel(key)}</dt>
						<dd>{valueWords(value)}</dd>
					</div>
				{/each}
			</dl>
		{:else}
			<div class="row"><span>Settings</span><span class="value">Not recorded</span></div>
		{/if}
	{:else}
		<div class="row"><span>Calculation</span><span class="value">No case open</span></div>
	{/if}
</div>

<style>
	.calculation-settings {
		padding: var(--tellegen-calculation-padding, 0);
		margin-bottom: 16px;
		font-size: var(--fs-body);
	}
	/* A calculation with no options of its own takes no room. */
	.calculation-settings:not(:has(h3, .row, .status > *)) {
		display: none;
	}
	h3 {
		margin: 16px 0 2px;
		font-size: var(--fs-label);
		font-weight: 600;
		color: var(--text-secondary);
	}
	h3:first-of-type {
		margin-top: 0;
	}
	fieldset {
		border: 0;
		padding: 0;
		margin: 0;
		min-width: 0;
	}
	/* One setting per row: the name on the left, its control on the right. */
	.row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		min-height: 34px;
		border-bottom: 1px solid var(--line);
	}
	.row > label,
	.row > span:first-child,
	dt {
		min-width: 0;
		color: var(--text-primary);
	}
	.value,
	dd {
		margin: 0;
		color: var(--text-secondary);
		text-align: right;
		overflow-wrap: anywhere;
	}
	/* Every control in the right column is one width, so the right edge lines up. The
	   select keeps the panels' shared chevron: only its colour is set here. */
	select,
	input:not([type='checkbox']) {
		flex: none;
		width: var(--settings-control-width, 150px);
		box-sizing: border-box;
		min-height: 28px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		background-color: var(--surface-control);
		color: var(--ink);
		font: inherit;
	}
	input:not([type='checkbox']) {
		padding: 3px 8px;
	}
	input[type='number'] {
		text-align: right;
		font-variant-numeric: tabular-nums;
	}
	input.switch {
		width: 16px;
		height: 16px;
		margin: 0;
		accent-color: var(--accent-fill);
	}
	details {
		border-bottom: 1px solid var(--line);
	}
	details .row:last-child {
		border-bottom: 0;
	}
	summary {
		display: list-item;
		line-height: 34px;
		cursor: pointer;
		color: var(--text-primary);
		list-style-position: inside;
	}
	summary:hover {
		color: var(--text-primary);
	}
	dl {
		margin: 0;
	}
	.status {
		display: flex;
		align-items: center;
		gap: 8px;
		min-height: 30px;
		color: var(--text-secondary);
	}
	.status:not(:has(> *)) {
		display: none;
	}
	.chip {
		padding: 1px 8px;
		border-radius: var(--radius-full);
		background: var(--surface-chip);
		color: var(--text-primary);
		font-size: var(--fs-label);
	}
	.error {
		margin: 4px 0;
		color: var(--text-danger);
		font-weight: 600;
	}
	.failure p {
		margin: 0 0 8px;
		font-size: var(--fs-label);
		color: var(--text-secondary);
		overflow-wrap: anywhere;
	}
	button.text {
		padding: 2px 6px;
		border: 0;
		border-radius: var(--radius-control);
		background: none;
		color: var(--text-accent);
		font: inherit;
		font-weight: 600;
		cursor: pointer;
	}
	button.text:hover {
		background: var(--surface-hover);
	}
	button:focus-visible,
	select:focus-visible,
	input:focus-visible,
	summary:focus-visible {
		outline: 2px solid var(--focus-ring);
		outline-offset: 2px;
	}
	@media (pointer: coarse) {
		.calculation-settings {
			font-size: 16px;
		}
		.row {
			min-height: 44px;
		}
		summary {
			line-height: 44px;
		}
		select,
		input:not([type='checkbox']),
		button.text {
			min-height: 36px;
			font-size: 16px;
		}
	}
</style>
