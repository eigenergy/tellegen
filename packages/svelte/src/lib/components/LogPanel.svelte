<script lang="ts">
	import { getAppState, getController } from '../context.svelte.js';
	import DiagnosticLogView from './DiagnosticLogView.svelte';
	import PanelFrame from './PanelFrame.svelte';
	const app = getAppState();
	const ctrl = getController();
	let open = $state(false);
	// A bus named in a finding selects that bus on the case on screen.
	function selectBus(bus: number) {
		if (app.activeLocal) void ctrl.selectLocalBus(app.activeLocal.id, bus);
		else if (app.activeCaseId) ctrl.selectBus(app.activeCaseId, bus);
	}
</script>

<PanelFrame id="log" title="Log" side="right" order={40} width={360} bind:open>
	<div class="log-body">
		<DiagnosticLogView onbus={selectBus} />
	</div>
</PanelFrame>

<style>
	.log-body {
		padding: 12px 14px;
	}
</style>
