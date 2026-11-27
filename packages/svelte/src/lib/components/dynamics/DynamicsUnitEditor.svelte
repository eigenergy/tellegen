<script lang="ts">
	/** The editor of one generating unit (experimental): make it static,
	 * give it a machine with controls or an inverter, change or remove one
	 * model, start from a published unit, edit parameters, and open a
	 * controller as a block diagram to change it or build one. Each change
	 * writes new DYR text (or a controller graph) and initializes the case
	 * again; the engine refuses what does not initialize. Used in the Models
	 * tab and in the popup a unit's glyph on the map opens.
	 * @block Unit editor */
	import { untrack } from 'svelte';
	import {
		applyUnitPreset,
		editDyr,
		GRAPH_PORTS,
		modelTemplate,
		rmsDynamize,
		setUnitModel,
		type BlockGraph,
		type GraphSlot,
		type InverterPart,
		type ModelOverride,
		type ModelRole
	} from '@tellegen/engine';
	import { getAppState, getController } from '../../context.svelte.js';
	import { parseDrafts, recordsSignature } from '../../dynamics/live-logic.js';
	import { unitKey as keyOf, unitKind, type UnitKind } from '../../dynamics/units.svelte.js';
	import ControllerGraphEditor from './ControllerGraphEditor.svelte';

	let { unitKey }: { unitKey: string } = $props();

	const app = getAppState();
	const dyn = getController().dynamics;
	const catalog = dyn.catalog;
	const att = $derived(dyn.active);
	const local = $derived(app.activeLocal);
	const unit = $derived(catalog.find(unitKey));
	const ROLES = ['machine', 'exciter', 'governor', 'stabilizer'] as const;
	/** What the unit is now, shown above any change. */
	const kind = $derived<UnitKind>(unit ? unitKind(unit) : 'static');
	// The letters match the unit glyphs on the map.
	const KIND_LABEL: Record<UnitKind, string> = {
		machine: 'G · Synchronous machine',
		inverter: 'I · Inverter-based unit',
		static: 'S · Static unit'
	};
	const currentModels = $derived(
		!unit
			? ''
			: kind === 'machine'
				? unit.records.map((r) => r.model).join(' · ')
				: kind === 'inverter'
					? unit.inverter.join(' · ')
					: 'power flow only'
	);

	let busy = $state(false);
	let error = $state<string | null>(null);
	let note = $state<string | null>(null);
	async function act(what: string, f: () => Promise<void>) {
		busy = true;
		error = null;
		note = null;
		try {
			await f();
			note = `${what}: initialized.`;
		} catch (e) {
			error = (e as Error).message;
		} finally {
			busy = false;
		}
	}
	const text = () => att?.text ?? '';
	// The machine + controls a "Replace with" / "Add" writes. A machine unit
	// starts from its own models (a model the library does not offer keeps the
	// default); an inverter or static unit starts from the default set.
	const DEFAULT_PICK = { machine: 'GENROU', exciter: 'SEXS', governor: 'TGOV1', stabilizer: '' };
	function startPick() {
		const p = { ...DEFAULT_PICK };
		if (!unit || unitKind(unit) !== 'machine') return p;
		for (const role of ROLES) {
			const model = unit.records.find((r) => r.role === role)?.model;
			if (!model) {
				if (role !== 'machine') p[role] = '';
			} else if (catalog.ofRole(role).some((m) => m.name === model)) p[role] = model;
		}
		return p;
	}
	let pick = $state({ ...DEFAULT_PICK });
	const makeDynamic = (machine: string) =>
		act(machine === 'REGCA1' ? 'Inverter' : 'Machine and controls', async () => {
			if (!unit || !local?.studyInputJson) return;
			const t = await rmsDynamize(local.studyInputJson, text(), {
				units: [[unit.bus, unit.machine_id]],
				machine,
				exciter: machine === 'REGCA1' ? null : pick.exciter || null,
				governor: machine === 'REGCA1' ? null : pick.governor || null,
				stabilizer: machine === 'REGCA1' ? null : pick.stabilizer || null,
				replace: true
			});
			await dyn.editDynamics(t, withoutUnit());
		});
	const makeStatic = () =>
		act('Static unit', async () => {
			if (!unit) return;
			let t = await setUnitModel(text(), unit.bus, unit.machine_id, 'machine', null);
			t = await setUnitModel(t, unit.bus, unit.machine_id, 'inverter', null);
			await dyn.editDynamics(t, withoutUnit());
		});
	const setModel = (role: ModelRole, model: string) =>
		act(`${PART_LABEL[role as InverterPart] ?? role} ${model || 'removed'}`, async () => {
			if (!unit) return;
			// A controller graph belongs to a machine's control slot; the parts of
			// an inverter carry none.
			const slot = (['exciter', 'governor', 'stabilizer'] as string[]).includes(role);
			await dyn.editDynamics(
				await setUnitModel(text(), unit.bus, unit.machine_id, role, model || null),
				slot ? withoutSlot(role as GraphSlot) : undefined
			);
		});

	// ----- inverter parts -----
	const PARTS: InverterPart[] = ['converter', 'electrical', 'plant'];
	const PART_LABEL: Record<InverterPart, string> = {
		converter: 'Converter',
		electrical: 'Electrical control',
		plant: 'Plant control'
	};
	const inverterRecords = $derived(unit?.inverter_records ?? []);
	/** Every record whose parameters the editor shows: machine and controls,
	 * then the inverter parts. */
	const paramRecords = $derived(unit ? [...unit.records, ...inverterRecords] : []);
	let presetId = $state('');
	const applyPreset = () =>
		act('Published unit', async () => {
			if (!unit || !presetId) return;
			await dyn.editDynamics(
				await applyUnitPreset(text(), unit.bus, unit.machine_id, presetId),
				withoutUnit()
			);
		});

	// ----- parameters -----
	let draft = $state<Record<string, string>>({});
	// Drafts belong to the records they were typed against: another unit, or
	// any change of this unit's records (a model swapped, a preset, an edit
	// applied), clears them, so no draft lands on a parameter that moved.
	const signature = $derived(unit ? recordsSignature(paramRecords) : '');
	$effect(() => {
		void unitKey;
		void signature;
		draft = {};
	});
	// The replacement picks follow the same rule: another unit, or a change of
	// this unit's models, starts them again from what the unit now has.
	$effect(() => {
		void unitKey;
		void signature;
		void kind;
		pick = untrack(startPick);
	});
	const draftCount = $derived(Object.keys(draft).length);
	const applyParams = () =>
		act('Parameters', async () => {
			if (!unit) return;
			// Blank or non-numeric input is refused with the parameter named;
			// the drafts stay for correction.
			const { edits, errors } = parseDrafts(draft, paramRecords);
			if (errors.length) throw new Error(errors.join('; '));
			if (edits.length) await dyn.editDynamics(await editDyr(text(), edits));
			draft = {};
		});

	// ----- controller block diagrams -----
	const models = $derived<ModelOverride[]>(att?.models ?? []);
	const inUse = (slot: GraphSlot) =>
		unit
			? (models.find((m) => keyOf(m) === keyOf(unit) && m.graph.slot === slot) ?? null)
			: null;
	const withoutSlot = (slot: GraphSlot) =>
		models.filter((m) => !(unit && keyOf(m) === keyOf(unit) && m.graph.slot === slot));
	const withoutUnit = () => models.filter((m) => !(unit && keyOf(m) === keyOf(unit)));
	let editing = $state<{ slot: GraphSlot; graph: BlockGraph; inUse: boolean } | null>(null);
	async function openSlot(slot: GraphSlot) {
		if (!unit) return;
		error = null;
		const current = inUse(slot);
		if (current) {
			editing = { slot, graph: JSON.parse(JSON.stringify(current.graph)), inUse: true };
			return;
		}
		const rec = unit.records.find((r) => r.role === slot);
		if (rec) {
			try {
				editing = { slot, graph: await modelTemplate(text(), rec.line), inUse: false };
			} catch (e) {
				error = (e as Error).message;
			}
			return;
		}
		const port = GRAPH_PORTS[slot][0];
		editing = {
			slot,
			inUse: false,
			graph: {
				slot,
				name: `my ${slot}`,
				nodes: [
					{ id: port, type: 'input', port, inputs: [], pos: [24, 40] },
					{ id: 'out', type: 'gain', k: 0, inputs: [port], pos: [220, 40] }
				],
				output: 'out'
			}
		};
	}
	const channelKey = $derived(
		unit
			? unit.machine_id.trim() === '1'
				? `${unit.bus}`
				: `${unit.bus}#${unit.machine_id.trim()}`
			: ''
	);
	/** Block outputs at the newest live sample, while the graph runs live. */
	const values = $derived.by((): Record<string, number> | null => {
		const run = dyn.live.run;
		if (!editing || !editing.inUse || !run) return null;
		const out: Record<string, number> = {};
		for (const n of editing.graph.nodes) {
			const s = run.series(`sig:${editing.slot}.${n.id}:${channelKey}`);
			if (s?.length) out[n.id] = s[s.length - 1];
		}
		return Object.keys(out).length ? out : null;
	});
</script>

{#if unit}
	<div class="unit dyn-form" aria-label={`Unit ${unit.bus} '${unit.machine_id.trim()}'`}>
		<h4>
			Unit {unit.bus} '{unit.machine_id.trim()}'
			<span class="meta">{unit.mbase.toFixed(0)} MVA · P {unit.pg.toFixed(1)} MW</span>
		</h4>
		<p class="current" data-testid="dyn-unit-kind">
			<span class={`kind-badge ${kind}`}>{KIND_LABEL[kind]}</span>
			<span class="mono">{currentModels}</span>
		</p>
		<!-- A change of the unit type is its own step: closed until opened,
		     except on a static unit, where adding models is the next step. -->
		<details class="card change" open={kind === 'static'}>
			<summary>
				<h5>{kind === 'static' ? 'Add models' : 'Change unit type'}</h5>
			</summary>
			<p class="dim hint">
				{#if kind === 'machine'}
					Starts from this unit's models. A replacement writes new records and initializes the
					case again.
				{:else if kind === 'inverter'}
					A machine + controls replaces the inverter records ({unit.inverter.join(', ')}).
				{:else}
					Pick a machine and its controls, or add an inverter.
				{/if}
			</p>
			<div class="picks">
				{#each ROLES as role (role)}
					<label
						><span class="cap">{role}</span>
						<select bind:value={pick[role]}>
							{#if role !== 'machine'}<option value="">none</option>{/if}
							{#each catalog.ofRole(role) as m (m.name)}<option value={m.name}>{m.name}</option
								>{/each}
						</select></label
					>
				{/each}
			</div>
			<div class="kind">
				<button
					disabled={busy}
					onclick={() => makeDynamic(pick.machine)}
					data-testid="dyn-make-dynamic"
					>{unit.records.length || unit.inverter.length ? 'Replace with' : 'Add'} machine + controls</button
				>
				<button disabled={busy} onclick={() => makeDynamic('REGCA1')}
					>{unit.inverter.length
						? 'Reset inverter'
						: unit.records.length
							? 'Replace with inverter'
							: 'Add inverter'}</button
				>
				<button
					class="ghost"
					disabled={busy || (!unit.records.length && !unit.inverter.length)}
					onclick={makeStatic}>Make static</button
				>
			</div>
			{#if catalog.presets.length}
				<div class="row preset">
					<select bind:value={presetId} aria-label="Published unit">
						<option value="">Start from a published unit…</option>
						{#each catalog.presets as p (p.id)}<option value={p.id}>{p.label}</option>{/each}
					</select>
					<button disabled={busy || !presetId} onclick={applyPreset}>Use</button>
				</div>
				{#if presetId}<p class="meta">
						{catalog.presets.find((p) => p.id === presetId)?.source}
					</p>{/if}
			{/if}
		</details>

		{#if unit.records.length}
			<section class="card">
			<h5>Models</h5>
			<table class="roles">
				<tbody>
					{#each ROLES as role (role)}
						{@const rec = unit.records.find((r) => r.role === role)}
						{@const graph = role === 'machine' ? null : inUse(role)}
						<tr>
							<th>{role}</th>
							<td>
								<select
									value={rec?.model ?? ''}
									disabled={busy}
									onchange={(e) => setModel(role, e.currentTarget.value)}
								>
									{#if role !== 'machine'}<option value="">none</option>{/if}
									{#each catalog.ofRole(role) as m (m.name)}<option value={m.name}>{m.name}</option
										>{/each}
								</select>
								{#if graph}<span class="graph">diagram “{graph.graph.name}” in use</span>{/if}
							</td>
							<td>
								{#if role !== 'machine'}
									<button
										class="ghost"
										title="Open as a block diagram"
										disabled={busy}
										onclick={() => openSlot(role)}
										>Diagram</button
									>
								{/if}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
			</section>
		{/if}

		{#if inverterRecords.length}
			{@const converter = inverterRecords.find((r) => r.role === 'converter')}
			{@const chain = PARTS.map((p) => inverterRecords.find((r) => r.role === p)?.model ?? '')}
			{@const others = inverterRecords.filter((r) => r.role === 'inverter')}
			<section class="card" data-testid="dyn-inverter-models">
				<h5>Models</h5>
				<table class="roles">
					<tbody>
						{#each PARTS as part (part)}
							{@const rec = inverterRecords.find((r) => r.role === part)}
							{@const offered = catalog.ofRole(part)}
							<tr>
								<th>{PART_LABEL[part]}</th>
								<td>
									<select
										value={rec?.model ?? ''}
										disabled={busy || !converter}
										aria-label={PART_LABEL[part]}
										onchange={(e) => setModel(part, e.currentTarget.value)}
									>
										{#if part === 'plant'}<option value="">none (constant current)</option
											>{:else if !rec}<option value="" disabled>missing</option>{/if}
										{#if rec && !offered.some((m) => m.name === rec.model)}<option
												value={rec.model}
												disabled>{rec.model} (not simulated)</option
											>{/if}
										{#each offered as m (m.name)}<option value={m.name} title={m.label}
												>{m.name}</option
											>{/each}
									</select>
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
				{#if others.length}
					<p class="dim">
						Other inverter records (not simulated): {others.map((r) => r.model).join(', ')}.
					</p>
				{/if}
				<p class="dim hint-below">
					{#if chain.join(' ') === 'REGCA1 REECB1 REPCA1'}
						Simulated as a renewable plant (REGCA1 + REECB1 + REPCA1).
					{:else if chain[0] === 'REGCA1' && chain[1] === 'REECB1' && !chain[2]}
						Without a plant controller the unit is a constant current at its power-flow
						output; add REPCA1 to simulate the plant.
					{:else}
						tellegen simulates the chain REGCA1 + REECB1 + REPCA1; with other models the unit
						holds its power-flow output.
					{/if}
				</p>
			</section>
		{/if}

		{#if paramRecords.some((r) => r.parameters.length)}
			<section class="card">
				<h5>Parameters</h5>
				{#each paramRecords.filter((r) => r.parameters.length) as r (r.line)}
					<details class="rec">
						<summary
							><span class="model">{r.model}</span>
							<span class="meta"
								>{PART_LABEL[r.role as InverterPart] ?? r.role} · line {r.line} · {r.parameters
									.length}</span
							></summary
						>
						<div class="params">
							{#each r.parameters as p, i (i)}
								{@const k = `${r.line}:${i}`}
								<label class:changed={k in draft && draft[k].trim() !== p.text} title={p.description ?? ''}
									><span class="pname"
										>{p.name}{#if p.unit}<span class="unit-of">{p.unit}</span>{/if}</span
									>
									<input
										class="mono"
										inputmode="decimal"
										spellcheck="false"
										value={draft[k] ?? p.text}
										oninput={(e) => (draft = { ...draft, [k]: e.currentTarget.value })}
									/></label
								>
							{/each}
						</div>
					</details>
				{/each}
				{#if draftCount}
					<div class="row apply">
						<span class="meta grow">{draftCount} edited</span>
						<button class="ghost" onclick={() => (draft = {})}>Discard</button>
						<button class="primary" disabled={busy} onclick={applyParams}
							>{busy ? 'Initializing…' : `Apply ${draftCount} change${draftCount === 1 ? '' : 's'}`}</button
						>
					</div>
				{/if}
			</section>
		{/if}
		{#if kind === 'static'}
			<p class="dim">The unit holds its power-flow output. Add models above to simulate it.</p>
		{:else if unit.inverter.length && !inverterRecords.length}
			<p class="dim">Inverter records: {unit.inverter.join(', ')}.</p>
		{/if}
		{#if busy}<p class="dim" role="status">Initializing…</p>{/if}
		{#if note}<p class="ok" role="status">{note}</p>{/if}
		{#if error}<p class="err" role="alert">{error}</p>{/if}
	</div>
{:else}
	<p class="dim">Loading the unit…</p>
{/if}

{#if editing && unit}
	<ControllerGraphEditor
		graph={editing.graph}
		title={`Unit ${unit.bus} '${unit.machine_id.trim()}' · ${editing.slot}`}
		{values}
		onapply={async (g) => {
			if (!unit || !editing) return;
			await dyn.editDynamics(text(), [
				...withoutSlot(editing.slot),
				{ bus: unit.bus, machine_id: unit.machine_id.trim(), graph: g }
			]);
			editing = { ...editing, graph: g, inUse: true };
		}}
		onrevert={editing.inUse
			? async () => {
					if (!editing) return;
					await dyn.editDynamics(text(), withoutSlot(editing.slot));
					editing = null;
				}
			: undefined}
		onclose={() => (editing = null)}
	/>
{/if}

<style>
	.unit {
		min-width: 0;
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: 8px;
		color: var(--text-primary);
	}
	p,
	h4,
	h5 {
		margin: 0;
	}
	h4 {
		display: flex;
		align-items: baseline;
		flex-wrap: wrap;
		gap: 4px 8px;
		font-size: var(--fs-label);
	}
	h5 {
		margin-bottom: 6px;
		color: var(--text-tertiary);
		font-size: var(--fs-meta);
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}
	.card {
		padding: 8px 10px;
		border: 1px solid var(--border-default);
		border-radius: var(--radius-sm);
		background: var(--surface-chip);
	}
	.current {
		display: flex;
		align-items: baseline;
		flex-wrap: wrap;
		gap: 4px 8px;
		font-size: var(--fs-control);
	}
	.kind-badge {
		padding: 1px 7px;
		border: 1px solid var(--border-default);
		border-radius: 999px;
		font-size: var(--fs-meta);
		font-weight: 600;
	}
	.kind-badge.static {
		color: var(--text-secondary);
	}
	.change summary {
		cursor: pointer;
		list-style: none;
	}
	.change summary::-webkit-details-marker {
		display: none;
	}
	.change summary h5 {
		display: inline;
	}
	.change summary h5::before {
		content: '▸ ';
	}
	.change[open] summary h5::before {
		content: '▾ ';
	}
	.change[open] summary {
		margin-bottom: 6px;
	}
	.hint {
		margin-bottom: 6px;
	}
	.hint-below {
		margin-top: 6px;
	}
	.meta,
	.dim {
		color: var(--text-tertiary);
		font-size: var(--fs-meta);
		font-weight: normal;
	}
	.kind,
	.row {
		display: flex;
		gap: 6px;
		flex-wrap: wrap;
		align-items: center;
	}
	.kind {
		margin-top: 8px;
	}
	.preset {
		margin-top: 8px;
		flex-wrap: nowrap;
	}
	.preset select {
		flex: 1;
	}
	.picks {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 6px 10px;
		font-size: var(--fs-control);
	}
	.picks label {
		display: grid;
		gap: 2px;
		min-width: 0;
	}
	.picks select,
	.roles select {
		width: 100%;
	}
	.preset {
		min-width: 0;
	}
	.preset select {
		min-width: 0;
	}
	.cap {
		color: var(--text-secondary);
		font-size: var(--fs-meta);
		text-transform: capitalize;
	}
	.roles {
		width: 100%;
		border-collapse: collapse;
		font-size: var(--fs-control);
	}
	.roles th {
		width: 1%;
		padding: 3px 10px 3px 0;
		text-align: left;
		font-weight: normal;
		color: var(--text-secondary);
		text-transform: capitalize;
		white-space: nowrap;
	}
	.roles td {
		padding: 3px 0;
	}
	.roles td:last-child {
		width: 1%;
		padding-left: 6px;
		text-align: right;
		white-space: nowrap;
	}
	.roles tr + tr {
		border-top: 1px solid var(--border-default);
	}
	.graph {
		display: block;
		color: var(--text-accent);
		font-size: var(--fs-meta);
	}
	.rec + .rec {
		border-top: 1px solid var(--border-default);
	}
	.rec summary {
		display: flex;
		align-items: baseline;
		gap: 8px;
		padding: 5px 0;
		cursor: pointer;
		font-size: var(--fs-control);
		list-style: none;
	}
	.rec summary::-webkit-details-marker {
		display: none;
	}
	.rec summary::before {
		content: '▸';
		color: var(--text-tertiary);
		font-size: var(--fs-meta);
		transition: transform var(--dur-fast, 120ms) var(--ease-out, ease-out);
	}
	.rec[open] summary::before {
		transform: rotate(90deg);
	}
	.rec summary:hover .model {
		color: var(--text-accent);
	}
	.model {
		font-family: var(--font-mono);
		font-weight: 600;
	}
	.params {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
		gap: 3px 12px;
		padding: 2px 0 8px 16px;
		font-size: var(--fs-control);
	}
	.params label {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 9ch;
		align-items: center;
		gap: 6px;
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
	.params input {
		width: 100%;
		min-height: 22px;
		padding: 1px 5px;
		text-align: right;
	}
	.params label.changed .pname {
		color: var(--text-accent);
	}
	.params label.changed input {
		border-color: var(--accent);
		background: var(--accent-soft);
	}
	.apply {
		position: sticky;
		bottom: -10px;
		margin: 4px -10px -8px;
		padding: 6px 10px;
		border-top: 1px solid var(--border-default);
		background: var(--surface-menu);
	}
	.grow {
		flex: 1;
	}
	button.primary {
		border-color: var(--accent);
		background: var(--accent);
		color: var(--paper);
		font-weight: 600;
	}
	button.primary:hover:not(:disabled) {
		border-color: var(--accent-bright, var(--accent));
		background: var(--accent-bright, var(--accent));
		color: var(--paper);
	}
	.mono {
		font-family: var(--font-mono);
	}
	.ok {
		color: var(--text-accent);
		font-size: var(--fs-control);
	}
	.err {
		color: var(--text-danger);
		overflow-wrap: anywhere;
		font-size: var(--fs-control);
	}
</style>
