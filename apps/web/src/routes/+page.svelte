<script lang="ts">
	import { TellegenShell } from '@tellegen/svelte';
	import UsageObserver from '$lib/analytics/UsageObserver.svelte';
	import SeoHead from '$lib/components/SeoHead.svelte';
	import StudyWorkspace from '$lib/studies/StudyWorkspace.svelte';
</script>

<SeoHead
	path="/"
	title="tellegen — reactive power flow visualization"
	description="Reactive visualization for power systems optimization. Drag demand and watch locational marginal prices and their adjoint sensitivities update from KKT columns, with exact DC OPF and SOCWR solves running in your browser in WebAssembly."
	socialDescription="Differentiable power flow and optimal power flow in the browser. Perturb demand and watch locational marginal prices and their adjoint sensitivities update from KKT columns."
/>

<UsageObserver />
<TellegenShell />
<StudyWorkspace />
