<script lang="ts">
	import { onMount, untrack, type Snippet } from 'svelte';
	import { getPanelLayout, type PanelSide } from '../panels.svelte.js';
	let {
		id,
		title,
		side,
		order = 0,
		width = 340,
		open = $bindable(false),
		children,
		headerActions,
		onopen
	}: {
		id: string;
		title: string;
		side: PanelSide;
		order?: number;
		width?: number;
		open?: boolean;
		children: Snippet;
		headerActions?: Snippet;
		onopen?: () => void;
	} = $props();
	const layout = getPanelLayout();
	onMount(() =>
		layout.register({
			id,
			title,
			side,
			order,
			width,
			open,
			children,
			headerActions,
			onopen,
			setOpen: (value) => {
				open = value;
			}
		})
	);
	$effect(() => {
		const values = { id, open, title, width };
		untrack(() => layout.update(values.id, values.open, values.title, values.width));
	});
</script>
