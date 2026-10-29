/** Report-ready vector export of the visible view with a title block. Pure: no DOM. */
export const TITLE_BLOCK_HEIGHT = 76;
export const MAX_SVG_SEGMENTS = 200_000;
const CHAR_PX = 6.7;

export interface LineSet {
	sources: ArrayLike<number>;
	targets: ArrayLike<number>;
	/** RGBA bytes per segment. */
	colors: ArrayLike<number>;
	count: number;
	/** Per-segment pixel widths; `width` applies when absent. */
	widths?: ArrayLike<number> | null;
	width?: number;
}

export interface SvgText {
	position: ArrayLike<number>;
	text: string;
	color?: string;
	dx?: number;
	dy?: number;
}

export interface SvgGlyph {
	position: ArrayLike<number>;
	text: string;
	/** Counter-clockwise degrees, as text layers use. */
	angle: number;
	color: string;
	size?: number;
}

export interface TitleBlock {
	left: string[];
	right: string[];
}

export interface SvgView {
	/** World bounds `[x0, y0, x1, y1]`, y down. */
	bounds: [number, number, number, number];
	scale: number;
	width: number;
	height: number;
	background: string;
	ink: string;
	muted: string;
	lineSets?: LineSet[];
	points?: {
		positions: ArrayLike<number>;
		colors: ArrayLike<number>;
		count: number;
		radius: number;
	} | null;
	markers?: { position: ArrayLike<number>; radius: number; fill?: string; stroke?: string }[];
	glyphs?: SvgGlyph[];
	texts?: SvgText[];
	title: TitleBlock;
}

const escapeXml = (v: string) =>
	v.replace(
		/[&<>"']/g,
		(c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c] as string
	);
const rgb = (c: ArrayLike<number>, i = 0) => `rgb(${c[i]} ${c[i + 1]} ${c[i + 2]})`;
const r1 = (v: number) => Math.round(v * 10) / 10;

/** Fit a title-block line into its column; monospace makes character count a safe width proxy. */
export function fitText(text: string, px: number): string {
	const n = Math.max(4, Math.floor((px - 24) / CHAR_PX));
	return text.length > n ? `${text.slice(0, n - 1)}…` : text;
}

/** Visible segments only, one path per colour and width. Throws rather than writing a partial
 * drawing when the view holds more than {@link MAX_SVG_SEGMENTS}. */
export function viewportSvg(view: SvgView): string {
	const [x0, y0, x1, y1] = view.bounds;
	const X = (x: number) => r1((x - x0) * view.scale);
	const Y = (y: number) => r1((y - y0) * view.scale);
	const inside = (p: ArrayLike<number>) => p[0] >= x0 && p[0] <= x1 && p[1] >= y0 && p[1] <= y1;
	const groups = new Map<string, string[]>();
	let segments = 0;
	for (const set of view.lineSets ?? []) {
		const { sources: s, targets: t, colors } = set;
		for (let i = 0; i < set.count; i++) {
			const [ax, ay, bx, by] = [s[i * 3], s[i * 3 + 1], t[i * 3], t[i * 3 + 1]];
			if (
				Math.max(ax, bx) < x0 ||
				Math.min(ax, bx) > x1 ||
				Math.max(ay, by) < y0 ||
				Math.min(ay, by) > y1
			)
				continue;
			if (++segments > MAX_SVG_SEGMENTS)
				throw new Error(
					`More than ${MAX_SVG_SEGMENTS.toLocaleString()} segments in view; zoom in to export a vector drawing`
				);
			if (colors[i * 4 + 3] === 0) continue;
			const key = `${rgb(colors, i * 4)}|${set.widths ? r1(set.widths[i]) : (set.width ?? 1)}|${r1(colors[i * 4 + 3] / 255)}`;
			const d = `M${X(ax)} ${Y(ay)}L${X(bx)} ${Y(by)}`;
			const group = groups.get(key);
			if (group) group.push(d);
			else groups.set(key, [d]);
		}
	}
	const paths = [...groups].map(([key, d]) => {
		const [stroke, width, alpha] = key.split('|');
		const opacity = Number(alpha) < 1 ? ` stroke-opacity="${alpha}"` : '';
		return `<path d="${d.join('')}" stroke="${stroke}" stroke-width="${width}"${opacity}/>`;
	});
	const dots: string[] = [];
	const points = view.points;
	if (points)
		for (let i = 0; i < points.count; i++) {
			const p = [points.positions[i * 3], points.positions[i * 3 + 1]];
			if (inside(p))
				dots.push(
					`<circle cx="${X(p[0])}" cy="${Y(p[1])}" r="${points.radius}" fill="${rgb(points.colors, i * 4)}"/>`
				);
		}
	for (const m of view.markers ?? [])
		if (inside(m.position)) {
			const stroke = m.stroke ? ` stroke="${m.stroke}" stroke-width="1.5"` : '';
			dots.push(
				`<circle cx="${X(m.position[0])}" cy="${Y(m.position[1])}" r="${m.radius}" fill="${m.fill ?? 'none'}"${stroke}/>`
			);
		}
	// Text-layer angles are counter-clockwise; SVG rotation is clockwise.
	const arrows = (view.glyphs ?? [])
		.filter((g) => inside(g.position))
		.map((g) => {
			const [x, y] = [X(g.position[0]), Y(g.position[1])];
			return `<text x="${x}" y="${y}" fill="${g.color}" font-size="${g.size ?? 22}" text-anchor="middle" dominant-baseline="central" transform="rotate(${r1(-g.angle)} ${x} ${y})">${escapeXml(g.text)}</text>`;
		});
	const labels = (view.texts ?? [])
		.filter((t) => inside(t.position))
		.map(
			(t) =>
				`<text x="${X(t.position[0]) + (t.dx ?? 0)}" y="${Y(t.position[1]) + (t.dy ?? 0)}" fill="${t.color ?? view.ink}">${escapeXml(t.text)}</text>`
		);
	const { width, height } = view;
	const column = width / 2;
	const line = (x: number, y: number, text: string, cls: string) =>
		`<text x="${x}" y="${y}" class="${cls}">${escapeXml(fitText(text, column))}</text>`;
	const block = [
		`<rect x="0" y="${height}" width="${width}" height="${TITLE_BLOCK_HEIGHT}" fill="${view.background}"/>`,
		`<line x1="0" x2="${width}" y1="${height + 0.5}" y2="${height + 0.5}" stroke="${view.muted}"/>`,
		...view.title.left.map((t, i) => line(12, height + 18 + i * 15, t, i ? 'muted' : 'strong')),
		...view.title.right.map((t, i) => line(column + 12, height + 18 + i * 15, t, 'muted'))
	];
	const total = height + TITLE_BLOCK_HEIGHT;
	return (
		`<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${total}" viewBox="0 0 ${width} ${total}" font-family="IBM Plex Mono, ui-monospace, monospace" font-size="11">` +
		`<style>path{fill:none;stroke-linecap:round}.muted{fill:${view.muted}}.strong{fill:${view.ink};font-size:12px;font-weight:600}</style>` +
		`<rect width="${width}" height="${height}" fill="${view.background}"/>` +
		`<g>${paths.join('')}</g><g>${dots.join('')}</g><g>${arrows.join('')}</g><g>${labels.join('')}</g>${block.join('')}</svg>`
	);
}

/** What was drawn, which view, and the provenance needed to reproduce it. */
export function titleBlock(t: {
	dataset: string;
	layout: string;
	view: string;
	camera: { target: ArrayLike<number>; zoom: number };
	result: {
		kind: 'solved' | 'stress';
		label: string;
		iterations: number | null;
		revision: string;
	} | null;
	exported: string;
}): TitleBlock {
	const r = t.result;
	return {
		left: [
			t.dataset,
			t.layout,
			t.view,
			`Centre ${t.camera.target[0].toFixed(1)}, ${t.camera.target[1].toFixed(1)} · ${(2 ** t.camera.zoom).toPrecision(3)}×`
		],
		right: [
			r?.kind === 'stress'
				? 'ARTIFICIAL STRESS: NOT A SOLUTION'
				: r
					? `${r.label}${r.iterations === null ? '' : ` · ${r.iterations} iterations`}`
					: 'No result snapshot',
			r ? `Result revision ${r.revision.slice(0, 16)}${r.revision.length > 16 ? '…' : ''}` : '',
			'P/Q positive into equipment at each end',
			`Exported ${t.exported}`
		].filter(Boolean)
	};
}
