<script lang="ts">
	import { resolve } from '$app/paths';
	import SeoHead from '$lib/components/SeoHead.svelte';
</script>

<SeoHead
	path="/changelog/"
	title="What's new in Tellegen"
	description="Tellegen updates: simpler saved Studies, Texas7k, movable panels, preserved maps and drawings, and agent tools."
/>
<main>
	<a href={resolve('/')}>Back to Tellegen</a>
	<h1>What's new in Tellegen</h1>
	<p class="date">September 2026 preview</p>
	<h2>Distribution AC power flow</h2>
	<p>
		Calculate terminal voltages and currents, compare voltages to ground or neutral, and save or
		export the result. Start with the 4-conductor example. This extends <a
			href="https://github.com/eigenergy/tellegen/pull/109"
			>Frederik Geth's multiconductor power-flow work</a
		>.
	</p>
	<h2>Quieter messages</h2>
	<p>
		Calculation and import errors appear briefly at the bottom. Repeated messages stay together;
		expand the recent list for details or retry.
	</p>
	<h2>Usage measurements</h2>
	<p>
		Anonymous counts and timings help improve case loading, calculations, Studies and agent tools.
		Uploaded data, study text and equipment values stay out of analytics. Disable measurements in <a
			href={resolve('/privacy')}>Privacy</a
		>.
	</p>
	<h2>Save a case, add a goal later</h2>
	<p>
		A Study is a saved case with its changes and results. Start with a name; add a planning goal
		when needed. Reopen it or export it to continue elsewhere.
	</p>
	<h2>Adjust demand and reset</h2>
	<p>
		Demand changes accumulate by bus. <strong>Reset to base case</strong> restores the original network
		inputs and keeps earlier changes and results in History.
	</p>
	<h2>Texas7k is back</h2>
	<p>
		The demo uses fitted generator costs. Expand <strong>Model details</strong> to see the approximation
		method and measured errors.
	</p>
	<h2>Arrange the workspace</h2>
	<p>
		Panels share left and right docks. Drag a heading to detach a panel, resize it, or use its menu
		to redock. <strong>Reset layout</strong> restores the default positions.
	</p>
	<h2>Keep maps and drawings</h2>
	<p>
		Saved Studies retain equipment positions, line paths and the current view. Geographic data
		appears on the map; drawing coordinates use a plain canvas with pan and zoom.
	</p>
	<h2>Let an agent use the same network</h2>
	<p>
		WebMCP tools can list and switch cases, answer network questions and inspect the saved state
		being displayed. Open <strong>Agent</strong>, then <strong>Connect</strong>, for a ready-to-copy
		request. Activity shows which tools were called.
	</p>
	<div class="links">
		<a href="https://eigenergy.github.io/tellegen/" target="_blank" rel="noreferrer"
			>Documentation</a
		><a href="https://github.com/eigenergy/tellegen/releases" target="_blank" rel="noreferrer"
			>Package release notes</a
		>
	</div>
</main>

<style>
	main {
		max-width: 700px;
		margin: 0 auto;
		padding: 48px 24px 80px;
		font: 16px/1.7 var(--font-display);
		color: var(--ink);
	}
	h1 {
		font-size: clamp(28px, 5vw, 40px);
		line-height: 1.2;
		margin: 28px 0 8px;
	}
	h2 {
		font-size: 21px;
		margin: 32px 0 10px;
	}
	p {
		margin: 10px 0;
	}
	a {
		color: #365747;
		text-underline-offset: 3px;
	}
	.date {
		color: var(--text-secondary);
		font: 12px var(--font-mono);
	}
	.links {
		display: flex;
		flex-wrap: wrap;
		gap: 20px;
		margin-top: 32px;
	}
</style>
