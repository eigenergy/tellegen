<script lang="ts">
	import { onMount } from 'svelte';
	import { PANEL_COMPACT_QUERY } from '../panels.svelte.js';
	import { getAppState, getController } from '../context.svelte.js';

	// Renderless. Keeps the shared app state in step with the environment: the
	// pointer that gates the file drop affordance, the compact-layout media
	// query, and the viewport height panel and map framing both read. The
	// default shell renders it; a custom shell on `PanelHost` must too.

	// Gates the drag-and-drop affordance only. The upload button in the header
	// is always rendered: a file picker works on every device, and the plain
	// `hover`/`pointer` queries resolve false on a touchscreen laptop that also
	// has a mouse, which hid the whole file flow there (#110). `any-*` match
	// when at least one attached input can hover and point precisely.
	const FILE_DROP_QUERY = '(any-hover: hover) and (any-pointer: fine)';

	const app = getAppState();
	const ctrl = getController();

	onMount(() => {
		const query = window.matchMedia(FILE_DROP_QUERY);
		const syncFileDropUi = () => {
			ctrl.showFileDropUi = query.matches;
			if (!ctrl.showFileDropUi) app.dragOver = false;
		};
		syncFileDropUi();
		query.addEventListener('change', syncFileDropUi);

		const compact = window.matchMedia(PANEL_COMPACT_QUERY);
		const syncCompact = () => {
			app.compactLayout = compact.matches;
			if (!compact.matches) app.sheetInset = 0;
		};
		syncCompact();
		compact.addEventListener('change', syncCompact);

		// Shared viewport measurements keep panel and map framing dimensions aligned.
		const syncViewport = () => (app.viewportHeight = window.innerHeight);
		syncViewport();
		window.addEventListener('resize', syncViewport);
		window.addEventListener('orientationchange', syncViewport);

		return () => {
			query.removeEventListener('change', syncFileDropUi);
			compact.removeEventListener('change', syncCompact);
			window.removeEventListener('resize', syncViewport);
			window.removeEventListener('orientationchange', syncViewport);
		};
	});
</script>
