<script lang="ts">
	import type { RmsRun } from '@tellegen/engine';
	import { getAppState, getController } from '../../context.svelte.js';
	import type { DynEvent } from '../../dynamics/events.js';
	import {
		coiRelativeDeg,
		envelope,
		largestSwings,
		lowest,
		sumPrefix,
		valueAt
	} from '../../dynamics/series.js';
	import TraceChart, { type TraceBand, type TraceSeries } from './TraceChart.svelte';

	let {
		run,
		events,
		height = 220
	}: { run: RmsRun; events: DynEvent[]; height?: number } = $props();

	const app = getAppState();
	const ctrl = getController();
	const dyn = ctrl.dynamics;
	type Tab = 'frequency' | 'angles' | 'voltages' | 'loads';
	let tab = $state<Tab>('frequency');
	let pinned = $state.raw<string[]>([]);
	let zoom = $state.raw<[number, number] | null>(null);
	let asTable = $state(false);
	const tEnd = $derived(run.time[run.time.length - 1]);
	const xRange = $derived<[number, number]>(zoom ?? [0, tEnd]);
	const f0 = $derived(run.meta.base_frequency);
	const base = $derived(run.meta.base_mva);
	const PALETTE = [
		'--series-1',
		'--series-2',
		'--series-3',
		'--series-4',
		'--series-5',
		'--series-6'
	];

	const units = $derived(
		run.meta.generators.map((g) =>
			g.machine_id.trim() === '1' ? String(g.bus) : `${g.bus}#${g.machine_id.trim()}`
		)
	);
	const tripAt = $derived(
		new Map(
			events
				.filter((e) => e.kind === 'generator_trip')
				.map((e) => [
					e.kind === 'generator_trip'
						? e.machineId.trim() === '1'
							? String(e.bus)
							: `${e.bus}#${e.machineId.trim()}`
						: '',
					e.at
				])
		)
	);
	const faultBuses = $derived(
		events.flatMap((e) =>
			e.kind === 'bus_fault' ? [e.bus] : e.kind === 'line_fault' ? [e.nearBus] : []
		)
	);

	// Derived once per run.
	const freqBand = $derived(envelope(run, 'f:'));
	const lowestF = $derived(lowest(run, 'f:', 5));
	const vBand = $derived(envelope(run, 'vm:'));
	const angles = $derived.by(() => {
		const out = new Map<string, Float64Array>();
		for (const u of units) {
			const r = coiRelativeDeg(run, u);
			if (!r) continue;
			const trip = tripAt.get(u);
			if (trip !== undefined) for (let i = 0; i < r.length; i++) if (run.time[i] > trip) r[i] = NaN;
			out.set(u, r);
		}
		return out;
	});
	const swings = $derived(largestSwings(run, units, 3));
	const pTotal = $derived(sumPrefix(run, 'pl:', base));
	const qTotal = $derived(sumPrefix(run, 'ql:', base));

	const label = (key: string) => {
		const [q, id] = key.split(':');
		const names: Record<string, string> = {
			f: 'bus',
			vm: 'bus',
			pl: 'load P',
			ql: 'load Q',
			delta: 'unit'
		};
		return `${names[q] ?? q} ${id}`;
	};

	const chart = $derived.by(
		(): {
			series: TraceSeries[];
			band: TraceBand | null;
			refLines: { value: number; label: string }[];
			unit: string;
			summary: string;
		} => {
			const pin = (prefix: string) =>
				pinned
					.map((k) => k.split(':')[1])
					.map((id, j) => ({ id, j }))
					.filter(({ id }) => run.series(`${prefix}${id}`));
			const s = run.screening;
			if (tab === 'frequency') {
				const series: TraceSeries[] = lowestF.map((k, j) => ({
					key: k,
					label: label(k),
					data: run.series(k)!,
					color: PALETTE[(j + 1) % PALETTE.length],
					width: 1.2,
					alpha: 0.8
				}));
				for (const { id, j } of pin('f:'))
					series.push({
						key: `f:${id}`,
						label: `bus ${id}`,
						data: run.series(`f:${id}`)!,
						color: PALETTE[j % 6],
						width: 2
					});
				series.push({
					key: 'coi:f',
					label: 'centre of inertia',
					data: run.series('coi:f')!,
					color: '--ink',
					width: 2.5
				});
				return {
					series,
					band: {
						lo: freqBand.lo,
						hi: freqBand.hi,
						color: '--series-1',
						label: 'range of all bus frequencies'
					},
					refLines:
						f0 === 60 ? [{ value: 59.5, label: '59.5 Hz (illustrative; UFLS not modelled)' }] : [],
					unit: 'Hz',
					summary: `Frequency: COI nadir ${s.frequency.coi_nadir_hz.toFixed(3)} Hz at ${s.frequency.nadir_t.toFixed(2)} s; ends at ${s.frequency.final_hz.toFixed(3)} Hz`
				};
			}
			if (tab === 'angles') {
				const series: TraceSeries[] = [];
				for (const [u, d] of angles)
					if (!swings.includes(u))
						series.push({
							key: `delta:${u}`,
							label: `unit ${u}`,
							data: d,
							color: '--ink-faint',
							width: 1,
							alpha: 0.25
						});
				swings.forEach((u, j) =>
					series.push({
						key: `delta:${u}`,
						label: `unit ${u}`,
						data: angles.get(u)!,
						color: PALETTE[j],
						width: 2
					})
				);
				return {
					series,
					band: null,
					refLines: [],
					unit: '° from COI',
					summary: `Rotor angles relative to the centre of inertia: largest separation ${s.angle.max_separation_deg.toFixed(1)}°${s.angle.pole_slips.length ? `; ${s.angle.pole_slips.length} unit(s) slip a pole` : ''}`
				};
			}
			if (tab === 'voltages') {
				const series: TraceSeries[] = faultBuses
					.filter((b) => run.series(`vm:${b}`))
					.map((b, j) => ({
						key: `vm:${b}`,
						label: `bus ${b} (faulted)`,
						data: run.series(`vm:${b}`)!,
						color: PALETTE[j % 6],
						width: 2
					}));
				for (const { id, j } of pin('vm:'))
					series.push({
						key: `vm:${id}`,
						label: `bus ${id}`,
						data: run.series(`vm:${id}`)!,
						color: PALETTE[(j + 2) % 6],
						width: 2
					});
				return {
					series,
					band: {
						lo: vBand.lo,
						hi: vBand.hi,
						color: '--series-2',
						label: 'range of all bus voltages'
					},
					refLines: [
						{ value: 0.8, label: '0.8 pu' },
						{ value: 0.9, label: '0.9 pu' }
					],
					unit: '|V| pu',
					summary: `Voltages: lowest ${s.voltage.min_pu.toFixed(3)} pu at bus ${s.voltage.min_bus ?? '–'}; final range ${s.voltage.final_min_pu.toFixed(3)}–${s.voltage.final_max_pu.toFixed(3)} pu`
				};
			}
			const series: TraceSeries[] = [
				{ key: 'sum:pl', label: 'total load P (MW)', data: pTotal, color: '--series-1', width: 2 },
				{ key: 'sum:ql', label: 'total load Q (Mvar)', data: qTotal, color: '--series-3', width: 2 }
			];
			for (const { id, j } of pin('pl:'))
				series.push({
					key: `pl:${id}`,
					label: `load P at ${id} (MW)`,
					data: run.series(`pl:${id}`)!.map((v) => v * base),
					color: PALETTE[(j + 3) % 6],
					width: 1.5
				});
			return {
				series,
				band: null,
				refLines: [],
				unit: 'MW / Mvar',
				summary: `Loads: total P from ${pTotal[0].toFixed(0)} MW to ${pTotal[pTotal.length - 1].toFixed(0)} MW`
			};
		}
	);

	const hoverKey = $derived(
		app.dynamicsHover
			? (chart.series.find((s) => s.key.endsWith(`:${app.dynamicsHover!.key}`))?.key ?? null)
			: null
	);

	function pick(key: string) {
		const id = key.split(':')[1];
		if (!id || key.startsWith('coi') || key.startsWith('sum')) return;
		const bus = Number(id.split('#')[0]);
		if (!pinned.includes(key)) pinned = [...pinned, key].slice(-6);
		const local = app.activeLocal;
		if (local && Number.isFinite(bus)) void ctrl.selectLocalBus(local.id, bus);
	}
	const tableRows = $derived.by(() => {
		if (!asTable) return [];
		const rows: { t: number; values: number[] }[] = [];
		const shown = chart.series.filter((s) => (s.alpha ?? 1) > 0.3).slice(0, 6);
		for (let t = 0; t <= tEnd + 1e-9; t += 0.5)
			rows.push({ t, values: shown.map((s) => valueAt(run.time, s.data, t)) });
		return rows;
	});
	const tableSeries = $derived(chart.series.filter((s) => (s.alpha ?? 1) > 0.3).slice(0, 6));
</script>

<div class="traces" data-testid="dyn-traces">
	<div class="bar">
		<div role="tablist" aria-label="Traces">
			{#each [['frequency', 'Frequency'], ['angles', 'Rotor angles'], ['voltages', 'Voltages'], ['loads', 'Loads']] as [id, name] (id)}
				<button role="tab" aria-selected={tab === id} onclick={() => (tab = id as Tab)}
					>{name}</button
				>
			{/each}
		</div>
		<div class="legend">
			{#each chart.series.filter((s) => (s.alpha ?? 1) > 0.3).slice(-6) as s (s.key)}
				<span class="chip"><i style:background="var({s.color})"></i>{s.label}</span>
			{/each}
			{#if chart.band}<span class="chip"
					><i class="band" style:background="var({chart.band.color})"></i>{chart.band.label}</span
				>{/if}
		</div>
		<div class="tools">
			{#if zoom}<button onclick={() => (zoom = null)}>Reset zoom</button>{/if}
			{#if pinned.length}<button onclick={() => (pinned = [])}>Unpin {pinned.length}</button>{/if}
			<button aria-pressed={asTable} onclick={() => (asTable = !asTable)}>Show as table</button>
		</div>
	</div>
	{#if asTable}
		<div class="table" style:max-height="{height}px">
			<table>
				<caption>{chart.summary}</caption>
				<thead
					><tr
						><th>t (s)</th>{#each tableSeries as s (s.key)}<th>{s.label}</th>{/each}</tr
					></thead
				>
				<tbody>
					{#each tableRows as r (r.t)}
						<tr
							><td>{r.t.toFixed(1)}</td>{#each r.values as v, i (i)}<td
									>{Number.isFinite(v) ? v.toFixed(4) : '–'}</td
								>{/each}</tr
						>
					{/each}
				</tbody>
			</table>
		</div>
	{:else}
		<TraceChart
			time={run.time}
			series={chart.series}
			band={chart.band}
			refLines={chart.refLines}
			unit={chart.unit}
			{xRange}
			playhead={dyn.player.t}
			highlight={hoverKey}
			ariaLabel={chart.summary}
			{height}
			onseek={(t) => {
				dyn.player.pause();
				dyn.player.seek(t);
			}}
			onhover={(key) => {
				if (!key || key.startsWith('coi') || key.startsWith('sum')) app.dynamicsHover = null;
				else {
					const id = key.split(':')[1];
					app.dynamicsHover = { kind: key.startsWith('delta') ? 'unit' : 'bus', key: id };
				}
			}}
			onpick={pick}
			onzoom={(r) => (zoom = r)}
		/>
	{/if}
</div>

<style>
	.traces {
		display: grid;
		gap: 4px;
	}
	.bar {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px;
		font-size: var(--fs-control);
	}
	[role='tablist'] {
		display: flex;
		gap: 2px;
	}
	button {
		padding: 3px 8px;
		border: 1px solid var(--border-default);
		border-radius: var(--radius-sm);
		background: var(--surface-chip);
		color: var(--ink);
		font: inherit;
		cursor: pointer;
	}
	button[aria-selected='true'],
	button[aria-pressed='true'] {
		border-color: var(--accent);
		box-shadow: var(--inset-active);
	}
	.legend {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		flex: 1;
		color: var(--text-secondary);
	}
	.chip {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		white-space: nowrap;
	}
	.chip i {
		display: inline-block;
		width: 12px;
		height: 3px;
		border-radius: 1px;
	}
	.chip i.band {
		height: 8px;
		opacity: 0.3;
	}
	.tools {
		display: flex;
		gap: 4px;
	}
	.table {
		overflow: auto;
		font-size: var(--fs-control);
	}
	table {
		border-collapse: collapse;
		font-variant-numeric: tabular-nums;
	}
	caption {
		text-align: left;
		color: var(--text-secondary);
	}
	th,
	td {
		padding: 1px 6px;
		text-align: right;
		border-bottom: 1px solid var(--line);
	}
</style>
