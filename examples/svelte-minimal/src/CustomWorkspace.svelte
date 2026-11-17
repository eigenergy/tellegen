<script lang="ts">
	import { DistributionResults, getAppState, getController } from '@tellegen/svelte';
	const app = getAppState();
	const ctrl = getController();
</script>

<label
	>Open a case <input
		type="file"
		accept=".pio.json,.dss,.json"
		onchange={(event) => {
			const files = event.currentTarget.files;
			if (files) void ctrl.ingestFiles(files);
		}}
	/></label
>
{#if app.error}<p role="alert">{app.error}</p>{/if}
{#if app.activeMulti}
	{@const c = app.activeMulti}
	<h2>{c.label}</h2>
	<button
		disabled={c.solving}
		onclick={() => void ctrl.solveMultiCase(c, c.mcOptions).catch(() => {})}
	>
		{c.solving ? 'Solving…' : 'Run power flow'}
	</button>
	<DistributionResults caseState={c} />
{:else}<p>Choose a distribution case to inspect.</p>{/if}
