<script lang="ts">
	import { TellegenShell } from '@tellegen/svelte';
	import UsageObserver from '#lib/analytics/UsageObserver.svelte';
	import SeoHead from '#lib/components/SeoHead.svelte';
	import StudyWorkspace from '#lib/studies/StudyWorkspace.svelte';
</script>

<SeoHead
	path="/"
	title="tellegen"
	description="Power flow and OPF in the browser."
	socialDescription="Power flow and OPF in the browser."
/>

<UsageObserver />
<TellegenShell />
<StudyWorkspace />
