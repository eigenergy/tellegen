import { getContext, setContext, type Snippet } from 'svelte';

export type PanelSide = 'left' | 'right';
export type PanelPosition = PanelSide | 'floating';
export interface PanelBounds {
	x: number;
	y: number;
	width: number;
	height: number;
}
export interface PanelRegistration {
	id: string;
	title: string;
	side: PanelSide;
	order: number;
	width: number;
	open: boolean;
	setOpen: (open: boolean) => void;
	children: Snippet;
	headerActions?: Snippet;
	onopen?: () => void;
}
interface SavedPanel extends PanelBounds {
	position: PanelPosition;
}
interface StoredLayout {
	version: 1;
	panels: Record<string, SavedPanel>;
}
const KEY = Symbol('tellegen.panel-layout');
const STORAGE_KEY = 'tellegen.panel-layout.v1';
export const PANEL_BREAKPOINT = 960;
export const PANEL_MIN_DOCK_HEIGHT = 500;
export const PANEL_COMPACT_QUERY = `(max-width: ${PANEL_BREAKPOINT}px), (max-height: ${PANEL_MIN_DOCK_HEIGHT - 1}px)`;

/** Layout belongs to one provider and stores presentation preferences only. */
export class PanelLayout {
	panels = $state.raw<PanelRegistration[]>([]);
	placements = $state<Record<string, SavedPanel>>({});
	compact = $state(false);
	drawer = $state<string | null>('network');
	lastActive = $state('network');
	dockActive = $state<Record<PanelSide, string>>({ left: 'network', right: 'solver' });
	viewportWidth = $state(1280);
	viewportHeight = $state(800);
	top = $state(100);
	storageError = $state<string | null>(null);
	#storage: Storage | undefined;
	#gesture: {
		panel: PanelRegistration;
		kind: 'move' | 'resize';
		x: number;
		y: number;
		bounds: PanelBounds;
		moved: boolean;
	} | null = null;
	beginGesture(
		panel: PanelRegistration,
		kind: 'move' | 'resize',
		x: number,
		y: number,
		bounds: PanelBounds
	) {
		this.#gesture = { panel, kind, x, y, bounds, moved: false };
	}
	moveGesture(x: number, y: number) {
		const gesture = this.#gesture;
		if (!gesture) return;
		const dx = x - gesture.x,
			dy = y - gesture.y;
		if (!gesture.moved && Math.hypot(dx, dy) < 4) return;
		gesture.moved = true;
		this.float(
			gesture.panel,
			gesture.kind === 'move'
				? {
						...gesture.bounds,
						x: gesture.bounds.x + dx,
						y: gesture.bounds.y + dy
					}
				: {
						...gesture.bounds,
						width: gesture.bounds.width + dx,
						height: gesture.bounds.height + dy
					}
		);
	}
	finishGesture() {
		if (this.#gesture?.moved) this.save();
		this.#gesture = null;
	}

	register(panel: PanelRegistration) {
		this.panels = [...this.panels.filter((p) => p.id !== panel.id), panel];
		return () => {
			this.panels = this.panels.filter((p) => p.id !== panel.id);
		};
	}
	update(id: string, open: boolean, title: string, width: number) {
		const previous = this.panels.find((p) => p.id === id);
		if (
			!previous ||
			(previous.open === open && previous.title === title && previous.width === width)
		)
			return;
		this.panels = this.panels.map((p) => (p.id === id ? { ...p, open, title, width } : p));
		if (open && !previous.open) {
			this.activate(previous);
			if (this.compact) this.drawer = id;
		}
	}
	activate(panel: PanelRegistration) {
		this.lastActive = panel.id;
		const side = this.position(panel);
		if (side !== 'floating') this.dockActive[side] = panel.id;
	}
	preferred(panel: PanelRegistration) {
		const side = this.position(panel);
		if (side === 'floating') return false;
		const open = this.ordered(side).filter((candidate) => candidate.open);
		const selected = open.find((candidate) => candidate.id === this.dockActive[side]) ?? open[0];
		return selected?.id === panel.id;
	}
	position(panel: PanelRegistration): PanelPosition {
		return this.placements[panel.id]?.position ?? panel.side;
	}
	ordered(side?: PanelSide) {
		return this.panels
			.filter((p) => side === undefined || this.position(p) === side)
			.sort((a, b) => a.order - b.order || a.id.localeCompare(b.id));
	}
	toggle(panel: PanelRegistration) {
		if (this.compact) {
			this.drawer = this.drawer === panel.id ? null : panel.id;
			if (this.drawer === panel.id) {
				this.activate(panel);
				panel.setOpen(true);
				panel.onopen?.();
			}
		} else {
			if (!panel.open) this.activate(panel);
			panel.setOpen(!panel.open);
			if (!panel.open) panel.onopen?.();
		}
	}
	close(panel: PanelRegistration) {
		panel.setOpen(false);
		if (this.drawer === panel.id) this.drawer = null;
	}
	dock(panel: PanelRegistration, side: PanelSide = panel.side) {
		this.placements[panel.id] = { ...this.bounds(panel), position: side };
		this.activate(panel);
		this.save();
	}
	float(panel: PanelRegistration, bounds?: PanelBounds) {
		this.placements[panel.id] = {
			...this.clamp(bounds ?? this.bounds(panel)),
			position: 'floating'
		};
	}
	bounds(panel: PanelRegistration): PanelBounds {
		return this.clamp(
			this.placements[panel.id] ?? {
				x: panel.side === 'left' ? 24 : this.viewportWidth - panel.width - 24,
				y: this.top + 16,
				width: panel.width,
				height: Math.min(520, this.viewportHeight - this.top - 110)
			}
		);
	}
	clamp(bounds: PanelBounds): PanelBounds {
		const availableWidth = Math.max(1, this.viewportWidth - 32);
		const availableHeight = Math.max(1, this.viewportHeight - this.top - 100);
		const width = Math.min(availableWidth, Math.max(240, bounds.width));
		const height = Math.min(availableHeight, Math.max(120, bounds.height));
		return {
			width,
			height,
			x: Math.max(16, Math.min(bounds.x, this.viewportWidth - width - 16)),
			y: Math.max(this.top, Math.min(bounds.y, this.viewportHeight - height - 88))
		};
	}
	resizeViewport(width: number, height: number, top: number) {
		this.viewportWidth = width;
		this.viewportHeight = height;
		this.top = top;
		const wasCompact = this.compact;
		this.compact = width <= PANEL_BREAKPOINT || height < PANEL_MIN_DOCK_HEIGHT;
		if (this.compact && !wasCompact && this.panels.length > 0) {
			const latest = this.panels.find((panel) => panel.id === this.lastActive && panel.open);
			this.drawer = latest?.id ?? this.panels.find((panel) => panel.open)?.id ?? null;
		}
		for (const [id, panel] of Object.entries(this.placements)) {
			if (panel.position === 'floating')
				this.placements[id] = { ...this.clamp(panel), position: 'floating' };
		}
	}
	dockWidth(side: PanelSide) {
		const panels = this.ordered(side).filter((p) => p.open);
		return panels.length
			? Math.min(Math.max(...panels.map((p) => p.width)), this.viewportWidth * 0.29)
			: 0;
	}
	reset() {
		this.placements = {};
		this.save();
	}
	initialize(storage?: Storage) {
		this.#storage = storage;
		if (!storage) return;
		try {
			const text = storage.getItem(STORAGE_KEY);
			if (!text) return;
			const value = JSON.parse(text) as StoredLayout;
			if (value.version !== 1 || !value.panels || typeof value.panels !== 'object') return;
			for (const [id, panel] of Object.entries(value.panels)) {
				if (
					!panel ||
					!['left', 'right', 'floating'].includes(panel.position) ||
					!['x', 'y', 'width', 'height'].every((key) =>
						Number.isFinite(panel[key as keyof PanelBounds])
					)
				)
					continue;
				this.placements[id] = {
					...this.clamp(panel),
					position: panel.position
				};
			}
		} catch {
			this.storageError = 'Panel layout could not be restored. Reset layout to use the defaults.';
		}
	}
	save() {
		if (!this.#storage) return;
		try {
			this.#storage.setItem(
				STORAGE_KEY,
				JSON.stringify({
					version: 1,
					panels: this.placements
				} satisfies StoredLayout)
			);
			this.storageError = null;
		} catch {
			this.storageError = 'Panel layout could not be saved. Changes last for this visit.';
		}
	}
}

export const setPanelLayout = (layout: PanelLayout): PanelLayout => setContext(KEY, layout);
export const getPanelLayout = (): PanelLayout => {
	const layout = getContext<PanelLayout | undefined>(KEY);
	if (!layout) throw new Error('PanelFrame requires a TellegenProvider');
	return layout;
};
