<script lang="ts">
	/** What the relays do, as it happens (experimental): the trips, breaker
	 * operations, recloses and lockouts of the run shown (at the player's
	 * time), the live session or a static study, newest first, over the map.
	 * @block Relay ticker */
	import type { RelayOperation } from '@tellegen/engine';
	import { getController } from '../../context.svelte.js';

	const prot = getController().dynamics.protection;
	const SHOWN = new Set<RelayOperation['kind']>([
		'trip',
		'breaker_open',
		'breaker_stuck',
		'breaker_failure',
		'reclose',
		'lockout',
		'blocked',
		'bypassed',
		'reinserted'
	]);
	const WORD: Record<string, string> = {
		trip: 'trips',
		breaker_open: 'breaker opens',
		breaker_stuck: 'breaker stuck',
		breaker_failure: 'breaker failure',
		reclose: 'recloses',
		lockout: 'locks out',
		blocked: 'blocked',
		bypassed: 'bypassed',
		reinserted: 'reinserted'
	};
	const shown = $derived.by(() => {
		const v = prot.view;
		if (!v || v.kind === 'planned' || !v.recorded) return null;
		const ops = v.ops.filter((o) => SHOWN.has(o.kind) && o.time <= v.t + 1e-9);
		return {
			t: v.t,
			trips: ops.filter((o) => o.kind === 'trip').length,
			rows: ops.slice(-7).reverse(),
			label: v.label
		};
	});
	let hidden = $state(false);
	const plural = (n: number) => `${n} trip${n === 1 ? '' : 's'}`;
</script>

{#if shown && !hidden}
	<div class="ticker" role="log" aria-label="Protection operations">
		<header>
			<span class="dot" class:hot={shown.trips > 0}></span>
			<b>Protection</b>
			<span class="dim"
				>{shown.label} · {Number.isFinite(shown.t) ? `t = ${shown.t.toFixed(3)} s · ` : ''}{plural(
					shown.trips
				)}</span
			>
			<button class="x" aria-label="Hide the protection operations" onclick={() => (hidden = true)}
				>×</button
			>
		</header>
		{#if shown.rows.length}
			<ol>
				{#each shown.rows as o, i (`${o.time}|${o.relay}|${o.kind}|${i}`)}
					<li class={o.kind} class:fresh={Number.isFinite(shown.t) && shown.t - o.time < 0.6}>
						<span class="t">{(o.time * 1000).toFixed(0)} ms</span>
						<button class="relay" onclick={() => (prot.selectedRelay = o.relay)}>{o.relay}</button>
						<span class="fn">{o.function === 'CB' ? '' : o.function}</span>
						<span class="what">{WORD[o.kind] ?? o.kind}</span>
					</li>
				{/each}
			</ol>
		{:else}
			<p class="dim quiet">No relay has operated yet.</p>
		{/if}
	</div>
{:else if shown && hidden}
	<button class="show" onclick={() => (hidden = false)}>Protection · {plural(shown.trips)}</button>
{/if}

<style>
	.ticker,
	.show {
		position: fixed;
		top: 136px;
		left: 50%;
		transform: translateX(-50%);
		z-index: 20;
		background: var(--glass, var(--surface-control));
		backdrop-filter: blur(var(--glass-blur, 10px));
		border-radius: var(--radius-md, 9px);
		box-shadow: var(--elev-2, 0 4px 16px rgba(0, 0, 0, 0.12));
		font-size: var(--fs-control);
		color: var(--ink);
	}
	.ticker {
		width: min(420px, calc(100vw - 32px));
		padding: 6px 10px 8px;
	}
	.show {
		border: none;
		padding: 4px 12px;
		font: inherit;
		cursor: pointer;
	}
	header {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.dim {
		color: var(--text-tertiary);
	}
	.quiet {
		margin: 4px 0 0;
	}
	.dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: rgb(46, 160, 90);
	}
	.dot.hot {
		background: rgb(227, 73, 72);
		box-shadow: 0 0 0 3px rgba(227, 73, 72, 0.25);
	}
	.x {
		margin-left: auto;
		border: none;
		background: none;
		font-size: 15px;
		color: var(--text-tertiary);
		cursor: pointer;
	}
	ol {
		list-style: none;
		margin: 4px 0 0;
		padding: 0;
	}
	li {
		display: grid;
		grid-template-columns: 4.2em 6.5em 4.5em 1fr;
		gap: 6px;
		align-items: baseline;
		padding: 2px 4px;
		border-radius: var(--radius-sm);
		font-family: var(--font-mono);
	}
	li.trip,
	li.lockout,
	li.breaker_failure,
	li.breaker_stuck {
		color: rgb(200, 45, 45);
		font-weight: 600;
	}
	li.reclose {
		color: rgb(30, 130, 70);
	}
	li.fresh {
		background: rgba(227, 73, 72, 0.12);
	}
	li.fresh.reclose {
		background: rgba(46, 160, 90, 0.14);
	}
	@media (prefers-reduced-motion: no-preference) {
		li.fresh {
			animation: flash 0.6s ease-out;
		}
	}
	@keyframes flash {
		from {
			background: rgba(227, 73, 72, 0.45);
		}
	}
	.t {
		color: var(--text-tertiary);
		font-weight: 400;
	}
	.relay {
		border: none;
		background: none;
		padding: 0;
		font: inherit;
		color: inherit;
		text-align: left;
		text-decoration: underline dotted;
		cursor: pointer;
	}
	.fn {
		font-weight: 400;
	}
</style>
