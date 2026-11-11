/** Equipment a result list row stands for, highlighted on the diagram while hovered or focused. */
export type PreviewItem = { kind: 'bus' | 'branch'; id: number };

/** Pointer and focus handlers that preview `item` on the diagram, to spread onto a list row. */
export function previewHandlers(
	onpreview: ((item: PreviewItem | null) => void) | undefined,
	item: PreviewItem
) {
	return {
		onpointerenter: () => onpreview?.(item),
		onpointerleave: () => onpreview?.(null),
		onfocus: () => onpreview?.(item),
		onblur: () => onpreview?.(null)
	};
}
