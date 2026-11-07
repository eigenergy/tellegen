/** Orthographic camera operations shared by diagram opening and fitting. */
export interface DiagramCamera {
	target: [number, number, number];
	zoom: number;
}

export interface Viewport {
	width: number;
	height: number;
}

export interface Insets {
	left: number;
	right: number;
	top: number;
	bottom: number;
}

export const OPENING_ZOOM_SLACK = 1;

/** Keep a useful close map view, but fit a feeder that would open as a speck. */
export function openingCamera(
	fromMap: DiagramCamera | null,
	fit: DiagramCamera,
	visible: (camera: DiagramCamera) => boolean
): DiagramCamera {
	return fromMap && fromMap.zoom >= fit.zoom - OPENING_ZOOM_SLACK && visible(fromMap)
		? fromMap
		: fit;
}

/** Visibility means a bus lies in the area not covered by panels or controls. */
export function showsAnyPosition(
	positions: ArrayLike<number>,
	camera: DiagramCamera,
	viewport: Viewport,
	insets: Insets
): boolean {
	const scale = 2 ** camera.zoom;
	for (let i = 0; i < positions.length; i += 3) {
		const x = viewport.width / 2 + (positions[i] - camera.target[0]) * scale;
		const y = viewport.height / 2 + (positions[i + 1] - camera.target[1]) * scale;
		if (
			x >= insets.left &&
			x <= viewport.width - insets.right &&
			y >= insets.top &&
			y <= viewport.height - insets.bottom
		)
			return true;
	}
	return false;
}

export function fitBounds(
	bounds: readonly [number, number, number, number],
	viewport: Viewport,
	insets: Insets,
	maxZoom: number
): DiagramCamera {
	const [x0, y0, x1, y1] = bounds;
	const width = Math.max(80, viewport.width - insets.left - insets.right);
	const height = Math.max(80, viewport.height - insets.top - insets.bottom);
	const zoom = Math.min(
		maxZoom,
		Math.log2(Math.min(width / Math.max(1, x1 - x0), height / Math.max(1, y1 - y0)))
	);
	// Apply the cap before centering: the offset is measured at the actual zoom.
	const scale = 2 ** zoom;
	return {
		target: [
			(x0 + x1) / 2 - (insets.left - insets.right) / 2 / scale,
			(y0 + y1) / 2 - (insets.top - insets.bottom) / 2 / scale,
			0
		],
		zoom
	};
}
