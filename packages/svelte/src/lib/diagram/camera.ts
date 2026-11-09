/** Orthographic camera arithmetic shared by the diagram canvases. A camera is a target in drawing
 * units and a zoom, 2^zoom pixels per unit; the free area is the part of the canvas no docked
 * panel or toolbar covers, as screen insets. Pure, so it is tested without a renderer. */

export interface DiagramCamera {
	target: [number, number, number];
	zoom: number;
}

export interface Insets {
	left: number;
	right: number;
	top: number;
	bottom: number;
}

export interface Viewport {
	width: number;
	height: number;
}

/** Zoom limits of the diagram canvases. `MAX_ZOOM` suits drawings in metres; use `maxZoomFor`
 * for a drawing whose units may be small. */
export const MIN_ZOOM = -24;
export const MAX_ZOOM = 8;

/** Pixels the drawing's longer side may span at the deepest zoom. */
const DEEPEST_SPAN_PX = 2 ** 16;

/** The deepest zoom for a drawing with these bounds: `MAX_ZOOM`, or deeper when the drawing's
 * units are small. Cases in diagram coordinates are laid out in a unit box, which `MAX_ZOOM`
 * alone would cap at 256 px across, too small to fit the canvas or to zoom into. */
export function maxZoomFor(bounds: [number, number, number, number]): number {
	const extent = Math.max(bounds[2] - bounds[0], bounds[3] - bounds[1]);
	if (!(extent > 0) || !Number.isFinite(extent)) return MAX_ZOOM;
	return Math.max(MAX_ZOOM, Math.log2(DEEPEST_SPAN_PX / extent));
}

/** The insets the panels leave free: docked panels on the sides, the toolbar on top, and the sheet
 * (compact layout) or the footer below. */
export function freeInsets(panels: {
	compact: boolean;
	top: number;
	dockWidth: (side: 'left' | 'right') => number;
	sheetInset: number;
}): Insets {
	return {
		left: panels.compact ? 28 : panels.dockWidth('left') + 44,
		right: panels.compact ? 28 : panels.dockWidth('right') + 44,
		top: panels.top + 64,
		bottom: panels.compact ? panels.sheetInset + 24 : 88
	};
}

/** The camera fitting `bounds` into the free area, centred between the panels, at most `maxZoom`. */
export function fitBounds(
	bounds: readonly [number, number, number, number],
	view: Viewport,
	free: Insets,
	maxZoom = MAX_ZOOM
): DiagramCamera {
	const [x0, y0, x1, y1] = bounds;
	const w = Math.max(80, view.width - free.left - free.right);
	const h = Math.max(80, view.height - free.top - free.bottom);
	const zoom = Math.min(
		maxZoom,
		Math.log2(Math.min(w / Math.max(1, x1 - x0), h / Math.max(1, y1 - y0)))
	);
	const scale = 2 ** zoom;
	return {
		target: [
			(x0 + x1) / 2 - (free.left - free.right) / 2 / scale,
			(y0 + y1) / 2 - (free.top - free.bottom) / 2 / scale,
			0
		],
		zoom
	};
}

/** How far (in zoom steps, factors of two) a carried-over camera may sit below the fitted one
 * before the diagram fits instead: further out, the drawing would span under half the free width. */
export const OPENING_ZOOM_SLACK = 1;

/** The camera a diagram opens with: the one carried over from the map when it shows the drawing
 * at a useful size, else the fitted one. A map zoomed in on part of the case keeps that view; a
 * map zoomed further out than the fit would open the drawing small in the middle of the canvas. */
export function openingCamera(
	carried: DiagramCamera | null,
	fitted: DiagramCamera,
	shows: (camera: DiagramCamera) => boolean
): DiagramCamera {
	return carried && carried.zoom >= fitted.zoom - OPENING_ZOOM_SLACK && shows(carried)
		? carried
		: fitted;
}

/** Where a drawing point lands on screen, in CSS pixels. */
export function toScreen(p: [number, number], camera: DiagramCamera, view: Viewport) {
	const scale = 2 ** camera.zoom;
	return [
		(p[0] - camera.target[0]) * scale + view.width / 2,
		(p[1] - camera.target[1]) * scale + view.height / 2
	] as [number, number];
}

/** The camera that brings `p` into the free area at zoom `zoom` (at least the current one), or
 * null when it is already visible at that zoom. The point is centred in the free area, not behind
 * a docked panel. */
export function revealPoint(
	p: [number, number],
	camera: DiagramCamera,
	view: Viewport,
	free: Insets,
	zoom = camera.zoom,
	maxZoom = MAX_ZOOM
): DiagramCamera | null {
	if (!Number.isFinite(p[0]) || !Number.isFinite(p[1])) return null;
	const next = Math.min(maxZoom, Math.max(camera.zoom, zoom));
	const [sx, sy] = toScreen(p, camera, view);
	const visible =
		sx > free.left &&
		sx < view.width - free.right &&
		sy > free.top &&
		sy < view.height - free.bottom;
	if (visible && next === camera.zoom) return null;
	const k = 2 ** next;
	return {
		target: [p[0] - (free.left - free.right) / 2 / k, p[1] - (free.top - free.bottom) / 2 / k, 0],
		zoom: next
	};
}

/** Whether `camera` shows at least one of the `x, y, z` positions. */
export function showsAnyPosition(
	positions: ArrayLike<number>,
	camera: DiagramCamera,
	view: Viewport,
	free: Insets = { left: 0, right: 0, top: 0, bottom: 0 }
): boolean {
	const scale = 2 ** camera.zoom;
	for (let i = 0; i < positions.length; i += 3) {
		const x = (positions[i] - camera.target[0]) * scale + view.width / 2;
		const y = (positions[i + 1] - camera.target[1]) * scale + view.height / 2;
		if (
			x >= free.left &&
			x <= view.width - free.right &&
			y >= free.top &&
			y <= view.height - free.bottom
		)
			return true;
	}
	return false;
}

/** The bounds of the given buses' positions, padded by `pad` units; null when none is finite. */
export function busBounds(
	positions: Float32Array,
	buses: Iterable<number>,
	pad = 0
): [number, number, number, number] | null {
	let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
	for (const i of buses) {
		const [x, y] = [positions[i * 3], positions[i * 3 + 1]];
		if (!Number.isFinite(x) || !Number.isFinite(y)) continue;
		[x0, y0, x1, y1] = [Math.min(x0, x), Math.min(y0, y), Math.max(x1, x), Math.max(y1, y)];
	}
	return Number.isFinite(x0) ? [x0 - pad, y0 - pad, x1 + pad, y1 + pad] : null;
}
