<script lang="ts">
	import { rgbaCss } from '../format.js';
	import { isPhaseTerminal, phaseColor } from '../multiconductor.js';
	import type { MultiWireView } from '../diagram/index.js';

	let { view }: { view: MultiWireView } = $props();

	const colour = (terminal: string) =>
		isPhaseTerminal(terminal) ? rgbaCss(phaseColor(terminal)) : 'var(--text-secondary)';
	const amps = (a: number | null) => (a === null ? '' : `${a.toFixed(a >= 100 ? 0 : 1)} A`);
</script>

<figure class="multi-wire">
	<svg
		viewBox="0 0 {view.width} {view.height}"
		role="img"
		aria-label="Multi-wire view of {view.kind} {view.id}: {view.wires
			.length} conductor segments between {view.buses[0].id} and {view.buses[1].id}"
	>
		{#each view.buses as bus, side (side)}
			<text class="bus" x={bus.x} y="12" text-anchor="middle">{bus.id}</text>
			{#each bus.terminals as t, r (r)}
				<g>
					{#if t.attachments.length}<title>{t.name}: {t.attachments.join(', ')}</title>{/if}
					<line
						class="rail"
						x1={bus.x - 8}
						x2={bus.x + 8}
						y1={t.y}
						y2={t.y}
						stroke={colour(t.name)}
					/>
					<text
						class="terminal"
						x={bus.x + (bus.x < view.width / 2 ? -12 : 12)}
						y={t.y + 3}
						text-anchor={bus.x < view.width / 2 ? 'end' : 'start'}>{t.name}</text
					>
					{#if t.grounded}
						<!-- Earth symbol: three shrinking bars below the rail end. -->
						<g
							class="earth"
							transform="translate({bus.x + (bus.x < view.width / 2 ? -26 : 26)} {t.y})"
						>
							<line x1="-4" x2="4" y1="4" y2="4" /><line
								x1="-2.6"
								x2="2.6"
								y1="6.5"
								y2="6.5"
							/><line x1="-1.2" x2="1.2" y1="9" y2="9" />
						</g>
					{/if}
				</g>
			{/each}
		{/each}
		{#if view.winding}
			<rect
				class="winding"
				x={view.winding.x}
				y={view.winding.y}
				width={view.winding.width}
				height={view.winding.height}
				rx="4"
			/>
			<text
				class="terminal"
				x={view.width / 2}
				y={view.winding.y + view.winding.height + 12}
				text-anchor="middle">windings</text
			>
		{/if}
		{#each view.wires as w, i (i)}
			<line
				class="wire"
				class:open={!view.closed}
				x1={w.x1}
				y1={w.y1}
				x2={w.x2}
				y2={w.y2}
				stroke={colour(w.terminal)}
			/>
			{#if w.amps !== null}
				<text class="amps" x={(w.x1 + w.x2) / 2} y={(w.y1 + w.y2) / 2 - 3} text-anchor="middle"
					>{amps(w.amps)}</text
				>
			{/if}
		{/each}
	</svg>
	<figcaption>
		Every conductor between the two terminal sets{view.winding
			? '; transformer windings couple magnetically, with no conductor continuity across them'
			: ''}{view.closed ? '' : '; open'}. Currents are magnitudes at the from end{view.winding
			? ' and the to end'
			: ''}. Hover a terminal for its attachments.
	</figcaption>
</figure>

<style>
	.multi-wire {
		margin: 8px 0 0;
	}
	svg {
		display: block;
		width: 100%;
		height: auto;
	}
	svg text {
		font: 9px var(--font-mono);
		fill: var(--text-secondary);
	}
	svg text.bus {
		fill: var(--ink);
	}
	.rail {
		stroke-width: 3;
	}
	.wire {
		stroke-width: 1.6;
	}
	.wire.open {
		stroke-dasharray: 4 3;
	}
	.winding {
		fill: var(--paper);
		stroke: var(--ink);
		stroke-width: 1;
	}
	.earth line {
		stroke: var(--text-secondary);
		stroke-width: 1;
	}
	figcaption {
		color: var(--text-secondary);
		font-size: 11px;
	}
</style>
