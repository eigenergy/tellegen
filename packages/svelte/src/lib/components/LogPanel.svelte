<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import { getPanelLayout } from '../panels.svelte.js';
	import DiagnosticLogView from './DiagnosticLogView.svelte';
	import PanelFrame from './PanelFrame.svelte';
	const app = getAppState();
	const ctrl = getController();
	const layout = getPanelLayout();
	let open = $state(false);
	// The tab counts the errors and warnings recorded since the Log was last on
	// screen. While it is on screen there is nothing to count.
	const shown = $derived.by(() => {
		const panel = layout.panels.find((candidate) => candidate.id === 'log');
		return !!panel && layout.shown(panel);
	});
	$effect(() => {
		if (!shown) return;
		app.log.markSeen();
		return () => app.log.markSeen();
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
	badge={shown ? undefined : app.log.unseen || undefined}
	bind:open
>
	<div class="log-body">
		<DiagnosticLogView onbus={selectBus} />
	</div>
</PanelFrame>

<style>
	.log-body {
		padding: 16px 18px;
	}
</style>
