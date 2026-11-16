<script lang="ts">
	/** The editor of one generating unit (experimental): make it static,
	 * give it a machine with controls or an inverter, change or remove one
	 * model, start from a published unit, edit parameters, and open a
	 * controller as a block diagram to change it or build one. Each change
	 * writes new DYR text (or a controller graph) and initializes the case
	 * again; the engine refuses what does not initialize. Used in the Models
	 * tab and in the popup a unit's glyph on the map opens. */
	import {
		applyUnitPreset,
		editDyr,
		GRAPH_PORTS,
		modelTemplate,
		rmsDynamize,
		setUnitModel,
		type BlockGraph,
		type GraphSlot,
		type ModelOverride,
		type ModelRole
	} from '@tellegen/engine';
	import { getAppState, getController } from '../../context.svelte.js';
	import { unitKey as keyOf } from '../../dynamics/units.svelte.js';
	import ControllerGraphEditor from './ControllerGraphEditor.svelte';

	let { unitKey }: { unitKey: string } = $props();

	const app = getAppState();
	const dyn = getController().dynamics;
	const catalog = dyn.catalog;
	const att = $derived(dyn.active);
	const local = $derived(app.activeLocal);
	const unit = $derived(catalog.find(unitKey));

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
	let pick = $state({ machine: 'GENROU', exciter: 'SEXS', governor: 'TGOV1', stabilizer: '' });
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
		act(`${role} ${model || 'removed'}`, async () => {
			if (!unit) return;
			await dyn.editDynamics(
				await setUnitModel(text(), unit.bus, unit.machine_id, role, model || null),
				role === 'machine' ? undefined : withoutSlot(role as GraphSlot)
			);
		});
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
	$effect(() => {
		void unitKey;
		draft = {};
	});
	const draftCount = $derived(Object.keys(draft).length);
	const applyParams = () =>
		act('Parameters', async () => {
			if (!unit) return;
			const edits = Object.entries(draft).flatMap(([k, t]) => {
				const [line, index] = k.split(':').map(Number);
				const value = Number(t);
				const p = unit.records.find((r) => r.line === line)?.parameters[index];
				return p && t.trim() !== p.text && Number.isFinite(value) ? [{ line, index, value }] : [];
			});
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
	const ROLES = ['machine', 'exciter', 'governor', 'stabilizer'] as const;
</script>

{#if unit}
	<div class="unit" aria-label={`Unit ${unit.bus} '${unit.machine_id.trim()}'`}>
		<h4>
			Unit {unit.bus} '{unit.machine_id.trim()}'
			<span class="dim">{unit.mbase.toFixed(0)} MVA · P {unit.pg.toFixed(1)} MW</span>
		</h4>
		<div class="kind">
			<button disabled={busy || (!unit.records.length && !unit.inverter.length)} onclick={makeStatic}
				>Static</button
			>
			<button disabled={busy} onclick={() => makeDynamic(pick.machine)} data-testid="dyn-make-dynamic"
				>{unit.records.length || unit.inverter.length ? 'Replace with' : 'Add'} machine + controls</button
			>
			<button disabled={busy} onclick={() => makeDynamic('REGCA1')}
				>{unit.inverter.length ? 'Reset inverter' : unit.records.length ? 'Replace with inverter' : 'Add inverter'}</button
			>
		</div>
		<div class="picks">
			{#each ROLES as role (role)}
				<label
					>{role}
					<select bind:value={pick[role]}>
						{#if role !== 'machine'}<option value="">none</option>{/if}
						{#each catalog.ofRole(role) as m (m.name)}<option value={m.name}>{m.name}</option>{/each}
					</select></label
				>
			{/each}
		</div>
		{#if catalog.presets.length}
			<div class="row">
				<select bind:value={presetId} aria-label="Published unit">
					<option value="">Start from a published unit…</option>
					{#each catalog.presets as p (p.id)}<option value={p.id}>{p.label}</option>{/each}
				</select>
				<button disabled={busy || !presetId} onclick={applyPreset}>Use</button>
			</div>
			{#if presetId}<p class="dim small">
					{catalog.presets.find((p) => p.id === presetId)?.source}
				</p>{/if}
		{/if}

		{#if unit.records.length}
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
									<button class="ghost" disabled={busy} onclick={() => openSlot(role)}
										>Block diagram</button
									>
								{/if}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>

			<h5>Parameters</h5>
			{#each unit.records as r (r.line)}
				<details>
					<summary>{r.model} <span class="dim">{r.role} · line {r.line}</span></summary>
					<div class="params">
						{#each r.parameters as p, i (i)}
							<label title={p.description ?? ''}
								><span>{p.name}{p.unit ? ` [${p.unit}]` : ''}</span>
								<input
									class="mono"
									value={draft[`${r.line}:${i}`] ?? p.text}
									oninput={(e) => (draft = { ...draft, [`${r.line}:${i}`]: e.currentTarget.value })}
								/></label
							>
						{/each}
					</div>
				</details>
			{/each}
			{#if draftCount}
				<div class="row">
					<button class="ghost" onclick={() => (draft = {})}>Discard</button>
					<button disabled={busy} onclick={applyParams}
						>{busy ? 'Initializing…' : `Apply ${draftCount} change${draftCount === 1 ? '' : 's'}`}</button
					>
				</div>
			{/if}
		{:else if unit.inverter.length}
			<p class="dim">Inverter records: {unit.inverter.join(', ')}.</p>
		{:else}
			<p class="dim">Static: the unit holds its power-flow output. Add models above to simulate it.</p>
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
		display: grid;
		gap: 6px;
	}
	p,
	h4,
	h5 {
		margin: 0;
	}
	.kind,
	.row {
		display: flex;
		gap: 6px;
		flex-wrap: wrap;
		align-items: center;
	}
	.picks {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 4px 8px;
		font-size: var(--fs-control);
	}
	.picks label {
		display: grid;
	}
	.roles {
		border-collapse: collapse;
		font-size: var(--fs-control);
	}
	.roles th {
		text-align: left;
		font-weight: normal;
		color: var(--text-tertiary);
		padding-right: 6px;
	}
	.roles td {
		padding: 1px 4px 1px 0;
	}
	.graph {
		color: var(--text-accent);
		font-size: var(--fs-meta);
	}
	.params {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 2px 8px;
		font-size: var(--fs-control);
	}
	.params label {
		display: grid;
	}
	.params input {
		min-width: 0;
		font: inherit;
	}
	.mono {
		font-family: var(--font-mono);
	}
	.dim {
		color: var(--text-tertiary);
	}
	.small {
		font-size: var(--fs-meta);
	}
	.ok {
		color: var(--text-accent);
	}
	.err {
		color: var(--text-danger);
		overflow-wrap: anywhere;
	}
	details summary {
		cursor: pointer;
	}
</style>
