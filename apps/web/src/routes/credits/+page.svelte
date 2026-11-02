<script lang="ts">
	import { PUBLIC_CARTO_BASEMAP_KEY } from '$app/env/public';
	import SeoHead from '#lib/components/SeoHead.svelte';
</script>

<SeoHead
	path="/credits/"
	title="Credits - tellegen"
	description="Credits for tellegen demo case data, software, map tiles, and open source sources used by the public browser demo."
/>

<main class="credits">
	<a class="back mono" href="/">back to tellegen</a>
	<h1>Credits</h1>
	<p>
		Tellegen uses synthetic public test cases and open source software. ACTIVSg and CATS data are
		staged by the operator. The small distribution example is included with the app.
	</p>

	<section>
		<h2>Case Data</h2>
		<h3>Distribution example and power flow</h3>
		<p>
			Frederik Geth at the University of Queensland contributed the multiconductor AC power-flow
			solver and the 4-conductor example. The <a href="/examples/four-wire.LICENSE.md"
				>example license</a
			> includes its attribution and reuse terms.
		</p>
		<h3>ACTIVSg Synthetic Grids</h3>
		<p>
			The ACTIVSg cases are synthetic grids from the
			<a href="https://electricgrids.engr.tamu.edu/" target="_blank" rel="noreferrer"
				>Texas A&amp;M Electric Grid Test Case Repository</a
			>. The case file headers request this citation:
		</p>
		<p class="citation">
			A. B. Birchfield, T. Xu, K. M. Gegner, K. S. Shetye, and T. J. Overbye, "Grid Structural
			Characteristics as Validation Criteria for Synthetic Networks," IEEE Transactions on Power
			Systems, 2017.
		</p>

		<h3>CATS California Test System</h3>
		<p>
			CATS is published by WISPO POP as a geographically accurate synthetic California grid. The
			source repository requests this citation:
		</p>
		<p class="citation">
			S. Taylor, A. Rangarajan, N. Rhodes, J. Snodgrass, B. C. Lesieutre, and L. A. Roald,
			"California Test System (CATS): A Geographically Accurate Test System Based on the California
			Grid," IEEE Transactions on Energy Markets, Policy and Regulation, vol. 2, no. 1, pp. 107-118,
			2024. doi:10.1109/TEMPR.2023.3338568.
		</p>
		<ul>
			<li>
				<a
					href="https://github.com/WISPO-POP/CATS-CaliforniaTestSystem"
					target="_blank"
					rel="noreferrer">CATS repository</a
				>
			</li>
			<li>CATS is distributed under the BSD 3 Clause license in its repository.</li>
		</ul>
	</section>

	<section>
		<h2>Software And Maps</h2>
		<ul>
			<li>
				<a href="https://github.com/eigenergy/powerio" target="_blank" rel="noreferrer">powerio</a>
				parses case files in Rust and WebAssembly.
			</li>
			<li>
				<a href="https://github.com/eigenergy/tellegen" target="_blank" rel="noreferrer">tellegen</a
				>
				provides the solver, sensitivity, and visualization framework.
			</li>
			<li>
				<a href="https://github.com/jkitchin/pounce" target="_blank" rel="noreferrer">POUNCE</a>
				0.13.0 provides the experimental nonlinear AC OPF solver under EPL-2.0. The bundled
				<a href="/experimental-acopf/POUNCE-EPL-2.0.txt">license</a>,
				<a href="/experimental-acopf/POUNCE-NOTICE.txt">notice</a>, and
				<a href="/experimental-acopf/POUNCE-SOURCE.json">source manifest</a>
				include the source archive locations and checksums.
			</li>
			<li>
				{#if PUBLIC_CARTO_BASEMAP_KEY}
					The basemap is
					<a href="https://carto.com/" target="_blank" rel="noreferrer">CARTO</a>'s, with data from
					<a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noreferrer"
						>OpenStreetMap contributors</a
					>. When that style cannot be loaded the map falls back to
					<a href="https://openfreemap.org/" target="_blank" rel="noreferrer">OpenFreeMap</a>, which
					uses the
					<a href="https://openmaptiles.org/" target="_blank" rel="noreferrer">OpenMapTiles</a>
					schema.
				{:else}
					The basemap is served by
					<a href="https://openfreemap.org/" target="_blank" rel="noreferrer">OpenFreeMap</a>, using
					the <a href="https://openmaptiles.org/" target="_blank" rel="noreferrer">OpenMapTiles</a>
					schema and data from
					<a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noreferrer"
						>OpenStreetMap contributors</a
					>. When that style cannot be loaded the map falls back to
					<a href="https://carto.com/" target="_blank" rel="noreferrer">CARTO</a>.
				{/if}
				The app adjusts OpenFreeMap's styles: roads start at zoom 8, and in the dark theme water and land
				are recoloured. Your browser requests tiles directly from the provider on screen; the attribution
				control names it.
			</li>
		</ul>
	</section>

	<section>
		<h2>Synthetic Grid Data</h2>
		<p>
			The engine's synthetic grid generator is compiled into this site's WebAssembly with these
			tables:
		</p>
		<ul>
			<li>
				National electricity demand from Ember's
				<a
					href="https://ember-energy.org/data/yearly-electricity-data/"
					target="_blank"
					rel="noreferrer">Yearly Electricity Data</a
				>, through Our World in Data's
				<a href="https://github.com/owid/energy-data" target="_blank" rel="noreferrer"
					>energy dataset</a
				>, under
				<a href="https://creativecommons.org/licenses/by/4.0/" target="_blank" rel="noreferrer"
					>CC BY 4.0</a
				>. The table keeps each country's latest year. Its population column is Our World in Data's
				series, compiled from the
				<a href="https://ourworldindata.org/population-sources" target="_blank" rel="noreferrer"
					>sources it lists</a
				>.
			</li>
			<li>
				Populated places and country areas from
				<a href="https://www.naturalearthdata.com/" target="_blank" rel="noreferrer"
					>Natural Earth</a
				>, in the public domain.
			</li>
			<li>
				National transmission voltage levels, compiled from the operator, regulator and standards
				documents each entry cites.
			</li>
		</ul>
	</section>
</main>

<style>
	.credits {
		min-height: 100dvh;
		max-width: 780px;
		margin: 0 auto;
		padding: 34px 20px 60px;
		color: var(--ink);
	}

	.back {
		display: inline-block;
		margin-bottom: 26px;
		color: var(--accent);
		font-size: 12px;
		text-decoration: none;
	}

	h1,
	h2,
	h3 {
		font-family: var(--font-display);
		letter-spacing: 0;
	}

	h1 {
		margin: 0 0 18px;
		font-size: 34px;
	}

	h2 {
		margin: 30px 0 12px;
		font-size: 18px;
	}

	h3 {
		margin: 22px 0 8px;
		font-size: 15px;
	}

	p,
	li {
		font-size: 15px;
		line-height: 1.65;
	}

	ul {
		margin: 0;
		padding-left: 20px;
	}

	.citation {
		margin: 8px 0 12px;
		padding-left: 14px;
		border-left: 2px solid var(--line);
		color: var(--ink-dim);
	}

	a {
		color: var(--accent);
	}

	@media (max-width: 560px) {
		.credits {
			padding: 24px 16px 48px;
		}

		h1 {
			font-size: 28px;
		}
	}
</style>
