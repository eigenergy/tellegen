<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { getNoticeCenter } from '../notices.svelte.js';
	import { getPanelLayout } from '../panels.svelte.js';
	import DiagnosticLogView from './DiagnosticLogView.svelte';
	import PanelFrame from './PanelFrame.svelte';
	const app = getAppState();
	const ctrl = getController();
	const notices = getNoticeCenter();
	const layout = getPanelLayout();
	let open = $state(false);
	// Messages that popped up since the Log was last on screen: the tab counts
	// them, instead of a floating button over the map.
	let seen = $state(0);
	const unseen = $derived(Math.max(0, notices.entries.length - seen));
	$effect(() => {
		const count = notices.entries.length;
		const panel = layout.panels.find((candidate) => candidate.id === 'log');
		if (count < seen || (panel && layout.shown(panel))) seen = count;
	});
	// A bus named in a finding selects that bus on the case on screen.
	function selectBus(bus: number) {
		if (app.activeLocal) void ctrl.selectLocalBus(app.activeLocal.id, bus);
		else if (app.activeCaseId) ctrl.selectBus(app.activeCaseId, bus);
	}
</script>

<PanelFrame
	id="log"
	title="Log"
	side="left"
	order={40}
	width={360}
	badge={unseen || undefined}
	bind:open
>
	<div class="log-body">
		<DiagnosticLogView onbus={selectBus} />
	</div>
</PanelFrame>

<style>
	.log-body {
		padding: 12px 14px;
	}
</style>
