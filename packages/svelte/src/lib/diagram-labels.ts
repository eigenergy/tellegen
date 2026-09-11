type PositionedLabel = { id: number; lon: number; lat: number; name?: string | null };
type Box = { left: number; top: number; right: number; bottom: number };

/** Place labels in screen pixels so zooming reveals equipment names as space opens. */
export function diagramLabels(
	buses: PositionedLabel[],
	scale: number,
	x: number,
	y: number,
	width: number,
	height: number,
	selected: number | null
): Set<number> {
	const visible = new Set<number>();
	const cells = new Map<string, Box[]>();
	const order =
		selected === null
			? buses
			: [
					...buses.filter((bus) => bus.id === selected),
					...buses.filter((bus) => bus.id !== selected)
				];
	for (const bus of order) {
		const label = bus.name || String(bus.id);
		const left = bus.lon * scale + x + 8;
		const bottom = bus.lat * scale + y - 7;
		const box = { left, bottom, right: left + label.length * 7 + 6, top: bottom - 16 };
		if (box.right < 0 || box.left > width || box.bottom < 0 || box.top > height) continue;
		const keys: string[] = [];
		let overlaps = false;
		for (
			let col = Math.floor(Math.max(0, box.left) / 64);
			col <= Math.floor(Math.min(width, box.right) / 64);
			col++
		) {
			for (
				let row = Math.floor(Math.max(0, box.top) / 64);
				row <= Math.floor(Math.min(height, box.bottom) / 64);
				row++
			) {
				const key = `${col},${row}`;
				keys.push(key);
				if (
					cells
						.get(key)
						?.some(
							(other) =>
								box.left < other.right &&
								box.right > other.left &&
								box.top < other.bottom &&
								box.bottom > other.top
						)
				)
					overlaps = true;
			}
		}
		if (overlaps) continue;
		visible.add(bus.id);
		for (const key of keys) cells.set(key, [...(cells.get(key) ?? []), box]);
	}
	return visible;
}
