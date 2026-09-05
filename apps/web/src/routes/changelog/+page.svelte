<script lang="ts">
	import { resolve } from '$app/paths';
	import SeoHead from '$lib/components/SeoHead.svelte';
</script>

<SeoHead
	path="/changelog/"
	title="What's new in Tellegen"
	description="New features in Tellegen: saved Studies, agent tools, demand edits and power-system comparisons."
/>
<main>
	<a href={resolve('/')}>Back to Tellegen</a>
	<h1>What's new in Tellegen</h1>
	<p class="date">September 2026</p>
	<h2>Saved studies</h2>
	<p>
		Studies save the goal, permitted interventions, exact operating points, explored alternatives
		and decisions. Reopen a Study or export it to continue with another agent.
	</p>
	<h2>Explore with an agent</h2>
	<p>
		WebMCP lets a connected browser agent inspect a network, run sensitivity analyses, compare saved
		states and propose capacity or demand changes. Open <strong>Agent</strong>, then
		<strong>Connect</strong>, on the map for instructions and an example request.
	</p>
	<h2>Change demand and return to the base case</h2>
	<p>
		Demand edits accumulate by bus. Studies retain the original network data, so a base-case reset
		can be solved, reviewed and applied without erasing earlier work. Inspecting a candidate changes
		the view; Apply chooses the operating point.
	</p>
	<h2>Compare exact results</h2>
	<p>
		Evaluate weighted locational marginal prices, voltage targets and intervention costs. Bounded
		searches use implicit derivatives to select trials, then verify candidates with exact solves.
		Evidence includes changes in constraints and prediction accuracy.
	</p>
	<p>
		PowerIO 0.11 provides the shared network and problem data. Supported calculations remain DC OPF,
		AC power flow and the SOCWR relaxation; available operations depend on the formulation.
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
