<script lang="ts">
	import { onDestroy } from 'svelte';
	import { getUiConfig } from '../../context.svelte.js';
	import TxControlResults from './TxControlResults.svelte';
	import {
		solveDcGrid,
		solveAcDc,
		solveJointAcDcModule,
		probeAcOpfWorker,
		type NetworkEquipment,
		type DcGridSolution
	} from '@tellegen/engine';
	let { equipment, moduleJson }: { equipment: NetworkEquipment; moduleJson?: string } = $props();
	let result = $state<DcGridSolution | null>(null);
	let error = $state('');
	let coupled = $state<Awaited<ReturnType<typeof solveAcDc>> | null>(null);
	let busy = $state(false);
	let automaticRegulators = $state(false);
	let jointAvailable = $state(false);
	const config = getUiConfig();
	let abort: AbortController | undefined;
	$effect(() => {
		const probe = new AbortController();
		if (config.acOpfWasmUrl)
			void probeAcOpfWorker(config.acOpfWasmUrl, undefined, probe.signal).then((value) => {
				if (!probe.signal.aborted) jointAvailable = value;
			});
		return () => probe.abort();
	});
	onDestroy(() => {
		generation++;
		abort?.abort();
	});
	let generation = 0;
	const topology = $derived(equipment.dc_topology);
	const positions = $derived(
		new Map(
			(topology?.nodes ?? []).map((id, i, all) => {
				const a = (2 * Math.PI * i) / Math.max(1, all.length);
				return [id, { x: 180 + 125 * Math.cos(a), y: 135 + 95 * Math.sin(a) }];
			})
		)
	);
	const short = (id: string) => id.split('/').at(-1) ?? id;
	const num = (n: number) => Number(n.toPrecision(6));
	async function solve(mode?: 'acpf' | 'dcopf' | 'acopf') {
		if (!equipment.dc_model) return;
		const request = ++generation;
		abort?.abort();
		abort = new AbortController();
		busy = true;
		result = null;
		coupled = null;
		error = '';
		try {
			if (mode && moduleJson) {
				const next =
					mode === 'acopf'
						? await solveJointAcDcModule(config.acOpfWasmUrl!, moduleJson, abort.signal)
						: await solveAcDc(moduleJson, mode, mode === 'acpf' && automaticRegulators);
				if (request === generation) {
					coupled = next;
					result = next.dc;
				}
			} else {
				const next = await solveDcGrid(equipment.dc_model);
				if (request === generation) result = next;
			}
		} catch (e) {
			if (request === generation) error = e instanceof Error ? e.message : String(e);
		} finally {
			if (request === generation) busy = false;
		}
	}
	$effect(() => {
		void equipment;
		void moduleJson;
		void automaticRegulators;
		abort?.abort();
		generation++;
		busy = false;
		result = null;
		coupled = null;
		error = '';
	});
</script>

{#if topology && topology.nodes.length}
	<details class="dc-topology">
		<summary>Detailed DC network · {topology.nodes.length} nodes</summary>
		<p>
			Source connections, including converter terminals. This DC graph is separate from AC
			synchronous islands. Crossings without a node dot are not connections.
		</p>
		{#if topology.nodes.length <= 48}
			<!-- svelte-ignore a11y_no_noninteractive_tabindex (Keyboard users need to scroll the topology horizontally.) -->
			<div class="drawing" tabindex="0" role="region" aria-label="Scrollable DC topology">
				<svg
					viewBox="0 0 360 380"
					role="img"
					aria-label="Detailed DC nodes, branches and converters"
				>
					{#each topology.links as link (link.id)}
						{@const a = link.from ? positions.get(link.from) : undefined}
						{@const b = link.to ? positions.get(link.to) : undefined}
						{#if a && b}
							<line
								x1={a.x}
								y1={a.y}
								x2={b.x}
								y2={b.y}
								stroke-dasharray={link.connected !== true ? '5 4' : undefined}
								><title
									>{link.id} · {link.kind} · {link.connected === null
										? 'state unknown'
										: link.connected
											? 'connected'
											: 'disconnected'}</title
								></line
							>
						{:else if a && link.kind === 'ground'}
							<path d={`M${a.x} ${a.y}v20m-8 0h16m-13 4h10m-7 4h4`}
								><title>{link.id}: explicit ground</title></path
							>
						{/if}
					{/each}
					{#each topology.converters as c, i (c.id)}
						{@const x = 40 + (i * 280) / Math.max(1, topology.converters.length - 1)}
						{#each [c.terminal1, c.terminal2] as terminal, end}
							{@const p = terminal ? positions.get(terminal) : undefined}
							{#if p}<line
									stroke-dasharray={(end ? c.connected2 : c.connected1) === true
										? undefined
										: '5 4'}
									x1={x + (end ? 10 : -10)}
									y1="290"
									x2={p.x}
									y2={p.y}
									><title
										>{(end ? c.connected2 : c.connected1) === null
											? 'Terminal state unknown'
											: (end ? c.connected2 : c.connected1)
												? 'Connected terminal'
												: 'Disconnected terminal'}</title
									></line
								>{/if}
						{/each}
						<rect x={x - 18} y="290" width="36" height="30" />
						<path d={`M${x - 18} 320l36 -30`} />
						<text {x} y="342" text-anchor="middle">{c.kind.toUpperCase()} {short(c.id)}</text>
						<text {x} y="362" text-anchor="middle">AC {c.ac_bus ?? '?'}</text>
						<title>{c.id}: AC bus {c.ac_bus ?? 'mapping unavailable'}</title>
					{/each}
					{#each topology.nodes as id (id)}
						{@const p = positions.get(id)!}
						<circle cx={p.x} cy={p.y} r="4" />
						<text x={p.x} y={p.y - 10} text-anchor="middle">{short(id)}</text>
					{/each}
				</svg>
			</div>
		{/if}
		<ul>
			{#each topology.links as link (link.id)}
				<li>
					{short(link.id)}: {link.from ? short(link.from) : 'unknown terminal'} ↔ {link.to
						? short(link.to)
						: link.kind === 'ground'
							? 'ground'
							: 'unknown terminal'} · {link.kind} · {link.connected === null
						? 'state unknown'
						: link.connected
							? 'connected'
							: 'disconnected'}{link.resistance_ohm === null
						? ''
						: ` · ${num(link.resistance_ohm)} Ω`}
				</li>
			{/each}
		</ul>
		{#if equipment.dc_model}
			<button onclick={() => solve()} disabled={busy}
				>{busy ? 'Solving DC network…' : 'Solve DC network'}</button
			>
			{#if moduleJson}
				<label class="control-option"
					><input type="checkbox" bind:checked={automaticRegulators} disabled={busy} />Include
					automatic tap, PST and shunt regulators in AC power flow</label
				>
				<button onclick={() => solve('acpf')} disabled={busy}>Solve with AC power flow</button>
				<button onclick={() => solve('dcopf')} disabled={busy}>Dispatch AC generation</button>
				<button onclick={() => solve('acopf')} disabled={busy || !jointAvailable}
					>Optimize joint AC/DC dispatch</button
				>
				{#if !jointAvailable}<p>
						Joint dispatch needs the optional nonlinear solver, which is unavailable in this
						session.
					</p>{/if}
				{#if busy && jointAvailable}<button
						onclick={() => {
							generation++;
							abort?.abort();
							busy = false;
						}}>Cancel study</button
					>{/if}
				<p>
					Studies use the original imported case. Diagram edits are not applied, and results below
					do not replace the main diagram result.
				</p>
			{/if}
			<details>
				<summary>Choose a study · controls and limits</summary>
				<ul>
					<li><strong>DC network:</strong> DC voltages, currents and losses only.</li>
					<li>
						<strong>AC power flow:</strong> applies converter P and Q to the AC network. A VSC voltage
						regulator adjusts Q; at a Q limit, voltage can miss its target. Optional tap and shunt controls
						use their declared ranges and steps.
					</li>
					<li>
						<strong>AC generation dispatch:</strong> a DC approximation of the AC network, with converter
						transfers fixed. It does not calculate AC voltage magnitudes or reactive power.
					</li>
					<li>
						<strong>Joint AC/DC dispatch:</strong> optimizes generation and converter active-power transfers
						together. DC voltage/current schedules and VSC AC-voltage targets are hard constraints; incompatible
						targets and limits can prevent a solution. Taps and shunt sections remain fixed.
					</li>
				</ul>
				<p>
					LCCs use an explicitly selected fixed power factor: Q = |P| tan(acos(pf)). Both rectifier
					and inverter absorb reactive power. This steady-state approximation does not model
					firing-angle control, commutation failure or transients.
				</p>
				<p>
					Joint dispatch finds a local nonlinear solution, not a guaranteed global optimum. It
					requires explicit converter P, DC-voltage and VSC Q bounds. Missing line ratings remain
					unchecked.
				</p>
			</details>
		{:else}
			<p class="notice">
				DC-network solve unavailable: {equipment.dc_model_unavailable ??
					'Required model data is missing'}.
			</p>
		{/if}
		{#if error}<p role="alert">{error}</p>{/if}
		{#if coupled}
			<section aria-label="AC/DC study results">
				<p>
					AC {coupled.ac.formulation?.toUpperCase() ?? 'network'} calculation completed · {coupled
						.violations.length} reported limit violations.
				</p>
				<p>{coupled.scope}</p>
				<TxControlResults controls={coupled.controls} />
				<p>Positive converter Mvar is absorbed from AC; negative Mvar is supplied to AC.</p>
				<ul>
					{#each coupled.converter_reactive as [id, q]}<li>{short(id)}: {num(q)} Mvar</li>{/each}
				</ul>
				{#if coupled.ac.objective != null}<p>
						AC generation objective: {num(coupled.ac.objective)}
					</p>{/if}
				<ul>
					{#each coupled.ac.vm ?? [] as v}<li>AC bus {v.bus}: {num(v.value)} pu</li>{/each}
				</ul>
				{#each coupled.violations as violation}<p class="notice">{violation}</p>{/each}
			</section>
		{/if}
		{#if result}
			<section aria-label="DC network results" aria-live="polite">
				<p>
					Converged in {result.iterations} iterations · {result.violations.length} limit violations.
				</p>
				<p>
					{result.limits_checked} declared bounds checked; {result.limits_unspecified} bounds unspecified.
					Convergence does not certify equipment feasibility.
				</p>
				<p>Positive converter MW is taken from the AC bus; negative MW is supplied to it.</p>
				{#each result.violations as violation}<p class="notice">{violation}</p>{/each}
				<ul>
					{#each result.nodes as node}<li>{short(node.id)}: {num(node.voltage_kv)} kV</li>{/each}
				</ul>
				<ul>
					{#each result.branches as b}<li>
							{short(b.id)}: {num(b.current_ka)} kA; {num(b.loss_mw)} MW line loss
						</li>{/each}
					{#each result.converters as c}<li>
							{short(c.id)}: {num(c.ac_power_mw)} MW from AC bus {c.ac_bus}; {num(c.loss_mw)} MW converter
							loss
						</li>{/each}
				</ul>
			</section>
		{/if}
	</details>
{/if}

<style>
	.control-option {
		display: flex;
		gap: 8px;
		align-items: flex-start;
		margin: 12px 0;
	}
	.control-option input {
		flex-shrink: 0;
		width: 18px;
		height: 18px;
	}
	summary {
		cursor: pointer;
		padding: 10px 0;
		font-weight: 600;
	}
	.drawing {
		overflow-x: auto;
		border: 1px solid #d2cec4;
	}
	svg {
		display: block;
		min-width: 250px;
		width: 100%;
		stroke: #555049;
		stroke-width: 1.4;
		fill: none;
	}
	text {
		fill: #37352f;
		stroke: none;
		font: 12px sans-serif;
	}
	circle {
		fill: #37352f;
	}
	rect {
		fill: #fbfaf6;
	}
	ul {
		padding-left: 18px;
		overflow-wrap: anywhere;
	}
	.notice {
		border-left: 3px solid #956b2a;
		padding-left: 8px;
	}
	button {
		min-height: 44px;
	}
</style>
