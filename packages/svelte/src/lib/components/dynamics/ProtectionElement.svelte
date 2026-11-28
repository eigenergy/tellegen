<script lang="ts">
	/** The relays of one element (a line or a bus with its busbar and
	 * units), inside that element's actions (experimental): their functions
	 * with the main settings (editable), in service, what they did in the run
	 * shown, the standard tests that concern the element, and a fault on it
	 * left to the relays, on the load flow (static study) or in RMS. Without
	 * relays, "protect by rule".
	 * @block Relays of an element */
	import type { ProtectionFunction, ProtectionTest, Relay, StaticFault } from '@tellegen/engine';
	import { getController } from '../../context.svelte.js';
	import { elementMatch, eventTouches, FUNCTION_CODE } from '../../dynamics/protection.svelte.js';
	import { numberError } from '../../dynamics/live-logic.js';

	let {
		target,
		what
	}: {
		target: { kind: 'branch'; from: number; to: number; circuit?: string } | { kind: 'bus'; bus: number };
		/** "this line", "this bus", ... */
		what: string;
	} = $props();
	const dyn = getController().dynamics;
	const prot = dyn.protection;

	const p = $derived(prot.on);
	const match = $derived(elementMatch(target));
	const relays = $derived(p ? p.scheme.relays.filter(match) : []);
	const ids = $derived(new Set(relays.map((r) => r.id)));
	const findings = $derived(
		(p?.audit ?? []).filter(
			(f) => (f.severity === 'error' || f.severity === 'warning') && ids.has(f.relay)
		)
	);
	const tests = $derived(
		(p?.tests ?? []).filter(
			(t) =>
				t.events.some((e) => eventTouches(e, target)) ||
				t.expect.some((x) => 'relay' in x && ids.has(x.relay))
		)
	);
	const outcome = (name: string) => p?.suite?.tests.find((o) => o.name === name);

	let busy = $state<string | null>(null);
	let error = $state<string | null>(null);
	async function run(label: string, f: () => Promise<unknown>) {
		busy = label;
		error = null;
		try {
			await f();
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			busy = null;
		}
	}

	// What the relays did in the run, study or live session shown, up to its time.
	const SHOWN = new Set(['trip', 'breaker_open', 'reclose', 'lockout', 'breaker_failure', 'blocked']);
	const ops = $derived.by(() => {
		const v = prot.view;
		if (!v) return [];
		return v.ops.filter((o) => o.time <= v.t + 1e-9 && ids.has(o.relay) && SHOWN.has(o.kind));
	});

	const LABEL: Record<string, string> = {
		distance: 'distance',
		differential: 'differential',
		overcurrent: 'overcurrent',
		earth_fault: 'earth fault',
		undervoltage: 'undervoltage',
		overvoltage: 'overvoltage',
		neutral_overvoltage: 'neutral overvoltage',
		underfrequency: 'underfrequency',
		overfrequency: 'overfrequency',
		rocof: 'RoCoF',
		overexcitation: 'V/Hz',
		reverse_power: 'reverse power',
		loss_of_field: 'loss of field',
		out_of_step: 'out of step',
		negative_sequence: 'negative sequence',
		thermal: 'thermal',
		fault_ride_through: 'ride-through'
	};
	const name = (f: ProtectionFunction) =>
		`${'label' in f && f.label ? f.label : (FUNCTION_CODE[f.type] ?? '')} ${LABEL[f.type] ?? f.type}`.trim();

	/** Every number in `v` is finite (no setting left empty or mistyped). */
	const finite = (v: unknown): boolean =>
		typeof v === 'number'
			? Number.isFinite(v)
			: Array.isArray(v)
				? v.every(finite)
				: v !== null && typeof v === 'object'
					? Object.values(v).every(finite)
					: true;
	const update = (r: Relay) => {
		if (!finite(r)) {
			error = 'enter a number';
			return;
		}
		return run('Auditing…', () => prot.updateRelay(r));
	};
	/** Replace function `k` of relay `r` with `next`. */
	const setFn = (r: Relay, k: number, next: ProtectionFunction) =>
		update({ ...r, functions: r.functions.map((f, i) => (i === k ? next : f)) });
	/** The typed number, or NaN (refused by `update`) when it is not one. */
	const num = (e: Event) => {
		const text = (e.currentTarget as HTMLInputElement).value;
		return numberError(text) ? NaN : Number(text);
	};

	/** Every scalar setting of a function, for the full editor. */
	const SKIP = new Set(['type', 'label', 'zones', 'teleprotection', 'power_swing_blocking', 'load_encroachment', 'points', 'k0']);
	const CHOICES: Record<string, string[]> = {
		curve: [
			'definite',
			'iec_standard_inverse',
			'iec_very_inverse',
			'iec_extremely_inverse',
			'iec_long_time_inverse',
			'ieee_moderately_inverse',
			'ieee_very_inverse',
			'ieee_extremely_inverse'
		],
		direction: ['forward', 'reverse', 'non_directional'],
		characteristic: ['mho', 'quadrilateral']
	};
	const fields = (f: ProtectionFunction) =>
		Object.entries(f).filter(([k, v]) => !SKIP.has(k) && v !== null && typeof v !== 'object');
	const setField = (r: Relay, k: number, f: ProtectionFunction, key: string, value: unknown) =>
		setFn(r, k, { ...f, [key]: value } as ProtectionFunction);
	let full = $state(false);

	// ----- fault here -----
	let engine = $state<'static' | 'rms'>('static');
	let at = $state(50);
	function fault(): StaticFault {
		const loc = Math.min(0.999, Math.max(0.001, at / 100));
		return target.kind === 'branch'
			? { kind: 'line', from: target.from, to: target.to, circuit: target.circuit ?? '1', location: loc }
			: { kind: 'bus', bus: target.bus };
	}
	const faultHere = () =>
		engine === 'static' ? prot.studyFault(fault()) : prot.faultRms(fault(), `Fault on ${what}`);
	const study = $derived(prot.study && prot.view?.kind === 'study' ? prot.study.study : null);
</script>

{#if prot.available}
	<section class="relaycard" class:busy={!!busy} aria-label="Protection of {what}">
		<h5>Protection</h5>
		{#if !p}
			<p class="dim">The protection layer is off (Dynamics panel, Protection tab).</p>
			<button
				class="action"
				disabled={!!busy || dyn.active?.status !== 'ready'}
				onclick={() => run('Setting relays…', () => prot.setLayer(true))}
				>Switch the protection layer on</button
			>
		{:else if !relays.length}
			<p class="dim">No relay on {what}.</p>
			<button
				class="action"
				disabled={!!busy}
				onclick={() => run('Setting relays…', () => prot.protectElement(match))}
				>Protect {what} by rule</button
			>
		{:else}
			{#each relays as r, ri (`${r.id}|${ri}`)}
				<div class="relay" class:off={r.enabled === false} class:selected={prot.selectedRelay === r.id}>
					<div class="head">
						<button
							class="id"
							title="draw its zones"
							onclick={() => (prot.selectedRelay = prot.selectedRelay === r.id ? null : r.id)}
							><span class="dot"></span>{r.id}</button
						>
						<span class="grow"></span>
						<label class="check"
							><input
								type="checkbox"
								checked={r.enabled ?? true}
								disabled={!!busy}
								onchange={(e) => update({ ...r, enabled: e.currentTarget.checked })}
							/> in service</label
						>
					</div>
					{#each r.functions as f, k (k)}
						<div class="fn">
							<span class="fname">{name(f)}</span>
							{#if f.type === 'distance'}
								<table>
									<tbody>
										{#each f.zones as z, zi (zi)}
											<tr>
												<td><span class="sw {z.name}"></span>{z.name}{z.direction === 'reverse' ? ' rev' : ''}</td>
												<td
													><input disabled={!!busy}
														type="number"
														step="0.1"
														aria-label="{z.name} reach"
														value={z.reach.toFixed(2)}
														onchange={(e) =>
															setFn(r, k, {
																...f,
																zones: f.zones.map((y, j) => (j === zi ? { ...y, reach: num(e) } : y))
															})}
													/> Ω</td
												>
												<td
													><input disabled={!!busy}
														type="number"
														step="0.05"
														aria-label="{z.name} delay"
														value={z.delay}
														onchange={(e) =>
															setFn(r, k, {
																...f,
																zones: f.zones.map((y, j) => (j === zi ? { ...y, delay: num(e) } : y))
															})}
													/> s</td
												>
											</tr>
										{/each}
									</tbody>
								</table>
								<label class="check"
									><input disabled={!!busy}
										type="checkbox"
										checked={!!f.teleprotection}
										onchange={(e) =>
											setFn(r, k, {
												...f,
												teleprotection: e.currentTarget.checked
													? { scheme: 'pott', zone: 'Z2', channel_time: 0.01 }
													: null
											})}
									/> POTT</label
								>
							{:else if full}
								<span class="inputs wrap">
									{#each fields(f) as [key, value] (key)}
										<label
											>{key.replace(/_/g, ' ')}
											{#if typeof value === 'boolean'}
												<input disabled={!!busy}
													type="checkbox"
													checked={value}
													onchange={(e) => setField(r, k, f, key, e.currentTarget.checked)}
												/>
											{:else if typeof value === 'number'}
												<input disabled={!!busy} type="number" step="any" {value} onchange={(e) => setField(r, k, f, key, num(e))} />
											{:else if CHOICES[key]}
												<select disabled={!!busy} {value} onchange={(e) => setField(r, k, f, key, e.currentTarget.value)}
													>{#each CHOICES[key] as c (c)}<option value={c}>{c.replace(/_/g, ' ')}</option>{/each}</select
												>
											{:else}
												<span class="mono">{String(value)}</span>
											{/if}</label
										>
									{/each}
								</span>
							{:else if 'pickup' in f && typeof f.pickup === 'number'}
								<span class="inputs">
									<label
										>pickup <input disabled={!!busy}
											type="number"
											step="any"
											value={f.pickup}
											onchange={(e) => setFn(r, k, { ...f, pickup: num(e) } as ProtectionFunction)}
										/></label
									>
									{#if 'delay' in f && typeof f.delay === 'number'}
										<label
											>delay <input disabled={!!busy}
												type="number"
												step="0.05"
												value={f.delay}
												onchange={(e) => setFn(r, k, { ...f, delay: num(e) } as ProtectionFunction)}
											/> s</label
										>
									{/if}
								</span>
							{/if}
						</div>
					{/each}
					{#if full}
						<div class="inputs wrap">
							<label
								>breaker time <input disabled={!!busy}
									type="number"
									step="0.01"
									value={r.breaker_time ?? 0.05}
									onchange={(e) => update({ ...r, breaker_time: num(e) })}
								/> s</label
							>
							<label
								><input disabled={!!busy}
									type="checkbox"
									checked={!!r.breaker_failure}
									onchange={(e) =>
										update({
											...r,
											breaker_failure: e.currentTarget.checked ? { delay: 0.15, current: 0.1 } : null
										})}
								/> 50BF</label
							>
							{#if r.breaker_failure}
								<label
									>delay <input disabled={!!busy}
										type="number"
										step="0.01"
										value={r.breaker_failure.delay ?? 0.15}
										onchange={(e) => update({ ...r, breaker_failure: { ...r.breaker_failure, delay: num(e) } })}
									/> s</label
								>
							{/if}
							{#if r.location.kind === 'line'}
								<label
									><input disabled={!!busy}
										type="checkbox"
										checked={!!r.autoreclose}
										onchange={(e) =>
											update({
												...r,
												autoreclose: e.currentTarget.checked
													? { dead_time: 0.5, reclaim: 5, shots: 1, sync_angle: 30 }
													: null
											})}
									/> 79</label
								>
								{#if r.autoreclose}
									<label
										>dead time <input disabled={!!busy}
											type="number"
											step="0.1"
											value={r.autoreclose.dead_time ?? 0.5}
											onchange={(e) => update({ ...r, autoreclose: { ...r.autoreclose, dead_time: num(e) } })}
										/> s</label
									>
									<label
										>shots <input disabled={!!busy}
											type="number"
											step="1"
											min="1"
											value={r.autoreclose.shots ?? 1}
											onchange={(e) => update({ ...r, autoreclose: { ...r.autoreclose, shots: num(e) } })}
										/></label
									>
									<label
										>sync angle <input disabled={!!busy}
											type="number"
											step="5"
											min="0"
											value={r.autoreclose.sync_angle ?? 30}
											onchange={(e) => update({ ...r, autoreclose: { ...r.autoreclose, sync_angle: num(e) } })}
										/> °</label
									>
								{/if}
							{/if}
						</div>
					{:else}
						<div class="extras">
							{#if r.breaker_failure}<span class="tag">50BF {r.breaker_failure.delay ?? 0.15} s</span>{/if}
							{#if r.autoreclose}<span class="tag">79 dead {r.autoreclose.dead_time ?? 0.5} s</span>{/if}
							<span class="tag">breaker {((r.breaker_time ?? 0.05) * 1000).toFixed(0)} ms</span>
						</div>
					{/if}
				</div>
			{/each}
			{#if ops.length}
				<ol class="ops">
					{#each ops as o, i (i)}
						<li class={o.kind}>
							<span class="t">{o.time.toFixed(3)} s</span>
							{o.relay}
							{o.function}
							{o.kind.replace(/_/g, ' ')}
						</li>
					{/each}
				</ol>
			{/if}
			{#if findings.length}
				<ul class="findings">
					{#each findings as f, i (i)}<li class={f.severity}>{f.check}: {f.message}</li>{/each}
				</ul>
			{/if}
			<div class="row">
				<label class="check"><input type="checkbox" bind:checked={full} /> all settings</label>
				<span class="grow"></span>
				<button
					class="small"
					disabled={!!busy}
					title="set these relays again by rule"
					onclick={() => run('Setting relays…', () => prot.protectElement(match))}>Reset by rule</button
				>
			</div>

			<h5>Fault on {what}</h5>
			<div class="row">
				<select bind:value={engine} aria-label="Calculation">
					<option value="static">Load flow</option>
					<option value="rms">RMS</option>
				</select>
				{#if target.kind === 'branch'}
					<label class="at"
						>at <input type="range" min="1" max="99" bind:value={at} aria-label="Fault location" />
						<span class="mono">{at} %</span></label
					>
				{/if}
				<button class="small" disabled={!!busy || dyn.running} onclick={() => run('Computing…', faultHere)}
					>Fault here</button
				>
			</div>
			{#if study && engine === 'static'}
				<ol class="ops">
					{#each study.trips as t, i (i)}<li class:mine={ids.has(t.relay)}>
							<span class="t">{(t.time * 1000).toFixed(0)} ms</span>
							{t.relay}
							{t.function}
						</li>{/each}
				</ol>
				{#if study.cleared_at != null}<p class="dim">
						isolated after {(study.cleared_at * 1000).toFixed(0)} ms
					</p>{/if}
			{/if}

			<h5>Tests of {what} <span class="dim">{tests.length}</span></h5>
			{#if tests.length}
				<button
					class="small"
					disabled={!!busy || prot.suiteRunning}
					onclick={() => run('Running tests…', () => prot.runSuite(tests))}>Run all (RMS)</button
				>
				<ul class="tests">
					{#each tests as t, ti (`${t.name}|${ti}`)}
						{@const o = outcome(t.name)}
						<li class:pass={o?.passed} class:fail={o && !o.passed}>
							<span class="mark">{o ? (o.passed ? '✓' : '✗') : '·'}</span>
							<span class="tname" title={t.description}>{t.name}</span>
							<button
								class="small"
								title="replay in RMS on the map"
								aria-label="Replay {t.name}"
								disabled={!!busy || dyn.running}
								onclick={() => run('Replaying…', () => prot.replay(t as ProtectionTest))}>▶</button
							>
						</li>
						{#if o && !o.passed}
							{#each o.checks.filter((c) => !c.passed) as c, i (i)}<li class="why">
									{c.expectation} — {c.observed}
								</li>{/each}
						{/if}
					{/each}
				</ul>
			{:else}
				<p class="dim">No standard test concerns {what}.</p>
			{/if}
		{/if}
		{#if busy}<p class="dim" role="status">{busy}</p>{/if}
		{#if error}<p class="err" role="alert">{error}</p>{/if}
	</section>
{/if}

<style>
	.relaycard {
		display: grid;
		gap: 6px;
		margin-top: 6px;
		padding-top: 8px;
		border-top: 1px solid var(--line);
		font-size: var(--fs-control);
	}
	.relaycard.busy {
		opacity: 0.7;
	}
	h5 {
		margin: 4px 0 0;
		font-size: var(--fs-control);
		color: var(--text-tertiary);
	}
	p {
		margin: 0;
	}
	.dim {
		color: var(--text-tertiary);
	}
	.err {
		color: var(--text-danger);
		overflow-wrap: anywhere;
	}
	.row,
	.head {
		display: flex;
		gap: 6px;
		align-items: center;
		flex-wrap: wrap;
	}
	.grow {
		flex: 1;
	}
	.action,
	.small {
		justify-self: start;
		padding: 2px 8px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		background: var(--surface-control);
		color: var(--ink);
		font: inherit;
		cursor: pointer;
	}
	.action:disabled,
	.small:disabled {
		color: var(--text-tertiary);
		cursor: default;
	}
	.relay {
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: 6px 8px;
		border: 1px solid var(--line);
		border-radius: var(--radius-sm);
	}
	.relay.selected {
		border-color: rgb(46, 160, 90);
		box-shadow: 0 0 0 2px rgba(46, 160, 90, 0.18);
	}
	.relay.off {
		opacity: 0.55;
	}
	.id {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		border: none;
		background: none;
		padding: 0;
		font: inherit;
		font-family: var(--font-mono);
		font-weight: 600;
		color: inherit;
		cursor: pointer;
	}
	.dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: rgb(46, 160, 90);
	}
	.check,
	.at {
		display: inline-flex;
		gap: 4px;
		align-items: center;
		color: var(--text-secondary);
	}
	.fn {
		display: flex;
		flex-wrap: wrap;
		gap: 4px 10px;
		align-items: baseline;
	}
	.fname {
		min-width: 9em;
		color: var(--text-secondary);
	}
	.inputs {
		display: inline-flex;
		gap: 8px;
	}
	.inputs.wrap {
		display: flex;
		flex-wrap: wrap;
		gap: 4px 12px;
	}
	.inputs.wrap label {
		display: inline-flex;
		gap: 4px;
		align-items: center;
		color: var(--text-secondary);
	}
	input[type='number'] {
		width: 5.2em;
		padding: 1px 4px;
		font: inherit;
	}
	table {
		border-collapse: collapse;
	}
	td {
		padding: 1px 8px 1px 0;
		white-space: nowrap;
	}
	.sw {
		display: inline-block;
		width: 10px;
		height: 4px;
		margin-right: 5px;
		border-radius: 2px;
		vertical-align: middle;
		background: #999;
	}
	.sw.Z1 {
		background: rgb(46, 160, 90);
	}
	.sw.Z2 {
		background: rgb(230, 168, 23);
	}
	.sw.Z3 {
		background: rgb(232, 112, 40);
	}
	.extras {
		display: flex;
		gap: 4px;
		flex-wrap: wrap;
	}
	.tag {
		padding: 0 6px;
		border-radius: var(--radius-control);
		background: var(--surface-chip);
		color: var(--text-secondary);
	}
	ul,
	ol {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	.ops li {
		font-family: var(--font-mono);
		color: var(--text-secondary);
	}
	.ops li.trip,
	.ops li.lockout,
	.ops li.breaker_failure,
	.ops li.mine {
		color: var(--text-danger);
		font-weight: 600;
	}
	.t {
		color: var(--text-tertiary);
		font-weight: 400;
	}
	.findings li.error {
		color: var(--text-danger);
	}
	.findings li.warning {
		color: var(--text-secondary);
	}
	.tests li {
		display: flex;
		gap: 6px;
		align-items: center;
		padding: 1px 0;
	}
	.tests li.why {
		padding-left: 1.4em;
		color: var(--text-danger);
	}
	.mark {
		width: 1em;
		text-align: center;
		color: var(--text-tertiary);
	}
	.pass .mark {
		color: #2ea05a;
	}
	.fail .mark {
		color: var(--text-danger);
	}
	.tname {
		flex: 1;
	}
	.mono {
		font-family: var(--font-mono);
	}
</style>
