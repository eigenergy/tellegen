<script lang="ts">
	import { getAppState, getController } from '../../context.svelte.js';
	import { markers, sinceEvent } from '../../dynamics/events.js';
	import { SPEEDS, type Speed } from '../../dynamics/player.svelte.js';
	import DynamicsTraces from './DynamicsTraces.svelte';

	let {
		dockHeight = $bindable(0),
		flush = false
	}: {
		dockHeight?: number;
		/** Sit flush on the bottom edge, under an open phone sheet. */
		flush?: boolean;
	} = $props();

	const app = getAppState();
	const ctrl = getController();
	const dyn = ctrl.dynamics;
	const player = dyn.player;
	const record = $derived(dyn.current);
	const run = $derived(record?.run ?? null);
	const f0 = $derived(run?.meta.base_frequency ?? 60);
	const tEnd = $derived(run ? run.time[run.time.length - 1] : 0);
	const marks = $derived(record ? markers(record.events, f0) : []);
	const step = $derived(run?.outputStep ?? 0.01);
	let showTraces = $state(true);
	let traceHeight = $state(160);

	/** The nearest event at or before t, and how long after it, in cycles. */
	const after = $derived.by(() => {
		const t = player.t;
		const prior = marks.filter((m) => m.t <= t + 1e-9);
		const m = prior.at(-1);
		if (!m) return null;
		const cycles = (t - m.t) * f0;
		const what =
			m.kind === 'fault'
				? 'fault'
				: m.kind === 'clear'
					? 'clearing'
					: m.kind === 'trip'
						? 'trip'
						: 'load step';
		return { cycles, since: sinceEvent(t - m.t, f0), what, label: m.label };
	});
	const valueText = $derived.by(() => {
		const s = dyn.summary;
		const a = after;
		const parts = [`${player.t.toFixed(3)} seconds`];
		if (a) parts[0] += `, ${a.cycles.toFixed(1)} cycles after ${a.what}`;
		if (s?.coiHz != null) parts.push(`system frequency ${s.coiHz.toFixed(2)} Hz`);
		if (s && Number.isFinite(s.minV))
			parts.push(`lowest voltage ${s.minV.toFixed(2)} pu at bus ${s.minBus}`);
		return parts.join('; ');
	});

	function onkey(e: KeyboardEvent) {
		const target = e.target as HTMLElement | null;
		if (target?.closest('input:not([type=range]), select, textarea, [contenteditable]')) return;
		if (!run || app.mode !== 'dynamics') return;
		const k = e.key;
		const move = (d: number) => {
			e.preventDefault();
			player.pause();
			player.seek(player.t + d);
		};
		if (k === ' ' && target?.closest('[data-dyn-timeline]')) {
			e.preventDefault();
			player.toggle();
		} else if (k === '[') {
			e.preventDefault();
			player.jump(-1);
		} else if (k === ']') {
			e.preventDefault();
			player.jump(1);
		} else if (target?.closest('[data-dyn-timeline]')) {
			if (k === 'ArrowLeft') move(e.shiftKey ? -0.1 : -step);
			else if (k === 'ArrowRight') move(e.shiftKey ? 0.1 : step);
			else if (k === 'Home') move(-player.t);
			else if (k === 'End') move(tEnd - player.t);
		}
	}
</script>

<svelte:window onkeydown={onkey} />
<svelte:document onvisibilitychange={() => document.hidden && player.pause()} />

{#if run && record}
	<section
		class={['timeline', 'dyn-form', { compact: app.compactLayout, flush }]}
		aria-label="Dynamics playback"
		data-dyn-timeline
		bind:offsetHeight={dockHeight}
	>
		<div class="controls">
			<button
				class="play"
				aria-label={player.playing ? 'Pause' : 'Play'}
				onclick={() => player.toggle()}
				data-testid="dyn-play">{player.playing ? '❚❚' : '▶'}</button
			>
			<label class="speed"
				><span class="sr-only">Playback speed</span>
				<select
					value={String(player.speed)}
					onchange={(e) => (player.speed = Number(e.currentTarget.value) as Speed)}
				>
					{#each SPEEDS as s (s)}<option value={String(s)}>{s}×</option>{/each}
				</select></label
			>
			<label class="slow" title="0.25× from 0.1 s before to 1.5 s after each event"
				><input type="checkbox" bind:checked={player.slowAroundEvents} /> Slow at events</label
			>
			<!-- Fixed width, the event on its own line: the scrubber beside it
			     must not move as the readout grows. -->
			<output class="time" data-testid="dyn-time"
				><span>Time {player.t.toFixed(3)} s</span>{#if after}<span class="dim"
						>{after.since} after {after.what}</span
					>{/if}</output
			>
			<div class="scrub">
				<input
					type="range"
					min="0"
					max={tEnd}
					{step}
					value={player.t}
					aria-label="Simulation time"
					aria-valuetext={valueText}
					data-testid="dyn-scrubber"
					oninput={(e) => {
						player.pause();
						player.seek(Number(e.currentTarget.value));
					}}
				/>
				<div class="marks" aria-hidden="true">
					{#each marks as m, i (i)}
						<span class={['mark', m.kind]} style:left="{(100 * m.t) / tEnd}%" title={m.label}
						></span>
					{/each}
				</div>
			</div>
			<span class="end">{tEnd} s</span>
			{#if !app.compactLayout}
				<button class="toggle" aria-expanded={showTraces} onclick={() => (showTraces = !showTraces)}
					>{showTraces ? 'Hide traces' : 'Traces'}</button
				>
			{/if}
		</div>
		{#if showTraces && !app.compactLayout}
			<button
				class="resize"
				aria-label="Resize traces (up and down arrows)"
				onkeydown={(e) => {
					if (e.key === 'ArrowUp') traceHeight = Math.min(innerHeight * 0.5, traceHeight + 20);
					if (e.key === 'ArrowDown') traceHeight = Math.max(160, traceHeight - 20);
				}}
				onpointerdown={(e) => {
					const y0 = e.clientY;
					const h0 = traceHeight;
					const move = (ev: PointerEvent) =>
						(traceHeight = Math.max(160, Math.min(innerHeight * 0.5, h0 + (y0 - ev.clientY))));
					const up = () => {
						removeEventListener('pointermove', move);
						removeEventListener('pointerup', up);
					};
					addEventListener('pointermove', move);
					addEventListener('pointerup', up);
				}}
			></button>
			<DynamicsTraces {run} events={record.events} height={traceHeight} />
		{/if}
	</section>
{/if}

<style>
	.timeline {
		position: fixed;
		left: var(--tellegen-dock-left, 20px);
		right: var(--tellegen-dock-right, 20px);
		bottom: 12px;
		z-index: var(--z-dock);
		display: grid;
		gap: 6px;
		padding: 8px 12px 10px;
		border: 1px solid var(--border-default);
		border-radius: var(--radius-sm);
		background: var(--panel);
		backdrop-filter: blur(6px);
		box-shadow: var(--elev-2, 0 4px 24px rgb(0 0 0 / 0.12));
		color: var(--ink);
		font-size: var(--fs-label);
	}
	.timeline.compact {
		left: 8px;
		right: 8px;
		bottom: 40px;
		padding: 4px 8px;
	}
	.timeline.compact.flush {
		left: 0;
		right: 0;
		bottom: 0;
		border-width: 1px 0 0;
		border-radius: 0;
		box-shadow: none;
		backdrop-filter: none;
	}
	.controls {
		display: flex;
		align-items: center;
		gap: 10px;
	}
	.play {
		width: 34px;
		height: 30px;
		border: 1px solid var(--accent-fill);
		border-radius: var(--radius-sm);
		background: var(--accent-fill);
		color: var(--on-accent);
		cursor: pointer;
		font-size: 12px;
	}
	select {
		padding: 2px 4px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		background: var(--surface-control);
		color: var(--ink);
		font: inherit;
	}
	.slow {
		display: flex;
		align-items: center;
		gap: 4px;
		font-size: var(--fs-control);
		color: var(--text-secondary);
		white-space: nowrap;
	}
	.time {
		display: grid;
		flex: none;
		width: 22ch;
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums;
		line-height: 1.2;
		white-space: nowrap;
	}
	.dim {
		overflow: hidden;
		text-overflow: ellipsis;
		color: var(--text-tertiary);
		font-size: var(--fs-meta);
	}
	.scrub {
		position: relative;
		flex: 1;
		min-width: 120px;
	}
	.scrub input {
		width: 100%;
		accent-color: var(--accent-fill);
	}
	.marks {
		position: absolute;
		inset: -10px 7px auto 7px;
		height: 12px;
		pointer-events: none;
	}
	/* An event is a short tick on the track: red for a fault, grey for clearing,
	   ink for a trip or a load change. */
	.mark {
		position: absolute;
		width: 2px;
		height: 10px;
		transform: translateX(-50%);
		background: var(--ink);
	}
	.mark.fault {
		background: var(--red);
	}
	.mark.clear {
		background: var(--text-tertiary);
	}
	.end {
		color: var(--text-tertiary);
		font-family: var(--font-mono);
	}
	.toggle {
		border: 1px solid var(--border-default);
		border-radius: var(--radius-sm);
		background: var(--surface-chip);
		color: var(--ink);
		font: inherit;
		cursor: pointer;
	}
	.resize {
		display: block;
		width: 100%;
		padding: 0;
		background: none;
		border: none;
		height: 6px;
		margin: -4px 0;
		cursor: ns-resize;
		border-top: 2px solid transparent;
	}
	.resize:hover,
	.resize:focus-visible {
		border-top-color: var(--accent);
	}
	.compact .slow,
	.compact .end,
	.compact .time .dim {
		display: none;
	}
	.compact .time {
		width: 14ch;
	}
</style>
