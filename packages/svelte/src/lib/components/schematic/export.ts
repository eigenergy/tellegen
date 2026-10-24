import type { Deck, OrthographicView } from '@deck.gl/core';
import type { RGBA } from '../../colors.js';
import { fitText, TITLE_BLOCK_HEIGHT, viewportSvg, type titleBlock } from '../../diagram/index.js';
import type { SchematicScene } from './layers.js';
import { segmentColours, segmentWidths, strokeColours } from './paint.js';
import { ACCENT, INK, PAPER, stateInk } from './palette.js';

type TitleBlock = ReturnType<typeof titleBlock>;

export interface ExportView {
	camera: { target: [number, number, number]; zoom: number };
	width: number;
	height: number;
}

/** Hand a file to the viewer. */
export function download(blob: Blob, name: string) {
	const url = URL.createObjectURL(blob);
	const a = document.createElement('a');
	a.href = url;
	a.download = name;
	a.click();
	setTimeout(() => URL.revokeObjectURL(url), 1000);
}

/** A file name stem from a case label. */
export const fileStem = (label: string) => `${label.replace(/[^\w.-]+/g, '_')}-schematic`;

/** The canvas as drawn, with a title block below it. */
export function canvasPng(
	deck: Deck<OrthographicView>,
	canvas: HTMLCanvasElement,
	title: TitleBlock
): Promise<Blob | null> {
	// Read the drawing buffer in the same task as a forced draw, before it is cleared.
	deck.redraw('export');
	const k = canvas.width / canvas.clientWidth;
	const out = document.createElement('canvas');
	const ctx = out.getContext('2d');
	if (!ctx) return Promise.resolve(null);
	out.width = canvas.width;
	out.height = canvas.height + TITLE_BLOCK_HEIGHT * k;
	ctx.fillStyle = 'rgb(252 251 247)';
	ctx.fillRect(0, 0, out.width, out.height);
	ctx.drawImage(canvas, 0, 0);
	for (const [col, lines] of [title.left, title.right].entries())
		lines.forEach((line, i) => {
			const lead = col === 0 && i === 0;
			ctx.font = `${lead ? '600 ' : ''}${(lead ? 12 : 11) * k}px "IBM Plex Mono", monospace`;
			ctx.fillStyle = lead ? 'rgb(32 36 43)' : 'rgb(77 81 86)';
			ctx.fillText(
				fitText(line, canvas.clientWidth / 2),
				(col * out.width) / 2 + 12 * k,
				canvas.height + (18 + i * 15) * k
			);
		});
	return new Promise((resolve) => out.toBlob(resolve, 'image/png'));
}

/** The visible region of a scene as vector SVG: lines, symbols, buses, the selection and labels. */
export function sceneSvg(s: SchematicScene, view: ExportView, title: TitleBlock): string {
	const { camera, width, height } = view;
	const d = s.drawn;
	const scale = 2 ** camera.zoom;
	const x0 = camera.target[0] - width / 2 / scale;
	const y0 = camera.target[1] - height / 2 / scale;
	const css = (c: RGBA) => `rgb(${c[0]} ${c[1]} ${c[2]})`;
	const symbols = s.symbols;
	return viewportSvg({
		bounds: [x0, y0, x0 + width / scale, y0 + height / scale],
		scale,
		width: Math.round(width),
		height: Math.round(height),
		background: css(PAPER),
		ink: css(INK),
		muted: 'rgb(77 81 86)',
		lineSets: [
			{
				sources: d.sources,
				targets: d.targets,
				colors: segmentColours(d, s.edgeColours, s.edgeMv, s.zoomedOut, s.fan),
				count: d.segmentEdge.length,
				widths: segmentWidths(d, s.edgeMv, s.colour !== 'none')
			},
			...(symbols
				? [
						{
							sources: symbols.sources,
							targets: symbols.targets,
							colors: strokeColours(symbols.owner, s.fan.edges, s.live),
							count: symbols.sources.length / 3,
							width: 1.25
						},
						...(s.ticksOn
							? [
									{
										sources: symbols.ticks.sources,
										targets: symbols.ticks.targets,
										colors: strokeColours(symbols.ticks.edge, s.fan.edges, s.live),
										count: symbols.ticks.edge.length,
										width: 1
									}
								]
							: [])
					]
				: [])
		],
		points: {
			positions: s.geometry.positions,
			colors: Uint8Array.from(s.busColours.flat()),
			count: s.graph.buses.length,
			radius: 2.6
		},
		markers:
			s.selectedBus >= 0
				? [
						{
							position: s.geometry.positions.subarray(s.selectedBus * 3, s.selectedBus * 3 + 2),
							radius: 7,
							stroke: css(ACCENT)
						}
					]
				: [],
		texts: s.labels.map((l) => ({
			position: l.position,
			text: l.text,
			dx: 6,
			dy: -8,
			color: css(stateInk(l.state))
		})),
		title
	});
}
