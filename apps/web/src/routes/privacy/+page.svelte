<script lang="ts">
	import { onMount } from 'svelte';
	import SeoHead from '$lib/components/SeoHead.svelte';
	import { browserAnalytics } from '$lib/analytics/client.js';
	let optedOut = $state(true);
	let browserBlocked = $state(false);
	let ready = $state(false);
	onMount(() => {
		const analytics = browserAnalytics()!;
		const update = () => {
			optedOut = analytics.optedOut;
			browserBlocked = analytics.blockedByBrowser;
			ready = true;
		};
		update();
		return analytics.subscribe(update);
	});
</script>

<SeoHead
	path="/privacy/"
	title="Privacy - tellegen"
	description="Local case files, saved studies, optional usage analytics, and map tile requests in Tellegen."
/>

<main class="privacy">
	<a class="back mono" href="/">Back to Tellegen</a>
	<h1>Privacy</h1>
	<section>
		<h2>Case files and saved studies</h2>
		<p>
			Dropped case files and coordinate files are processed in the browser. Their contents stay on
			the device. Saved studies, changes, and results use this browser's local storage. Exporting
			creates a local download.
		</p>
		<p>
			The server receives ordinary page and API requests for the public demo cases. Loading a local
			file does not upload it.
		</p>
	</section>
	<section>
		<h2>Usage analytics</h2>
		<p>
			Tellegen uses <a href="https://umami.is/privacy" rel="noreferrer">Umami Cloud</a> to understand
			page visits, use of public demos, actions, completion status, counts, calculation timings, and page
			speed.
		</p>
		<p>
			Analytics exclude uploaded filenames and contents, study names and goals, equipment
			identities, coordinates, electrical values, agent prompts, and error text. There is no screen
			recording or replay, and Tellegen does not assign persistent visitor IDs.
		</p>
		<p>
			Page addresses are reduced to fixed public pages. Search parameters, fragments, and referring
			addresses are excluded. Umami receives the browser's network request; its privacy policy
			describes the service's processing.
		</p>
		<label class="analytics-choice"
			><input
				type="checkbox"
				disabled={!ready || browserBlocked}
				checked={optedOut || browserBlocked}
				onchange={(event) => browserAnalytics()?.setOptOut(event.currentTarget.checked)}
			/>Disable usage analytics in this browser</label
		>
		<p class="preference" role="status">
			{browserBlocked
				? 'Your browser privacy setting disables analytics.'
				: optedOut
					? 'Analytics are disabled in this browser.'
					: 'Analytics are enabled on the public site.'}
		</p>
		<p>
			Do Not Track and Global Privacy Control are respected. Local and preview addresses do not load
			analytics. This preference is stored only in this browser.
		</p>
	</section>
	<section>
		<h2>Map tiles</h2>
		<p>
			The geographic basemap loads directly from CARTO. Tile requests reveal the map area, IP
			address, and browser details to that service. Case files and equipment data are not included.
			The plain diagram view does not request a basemap.
		</p>
		<p><a href="https://carto.com/privacy" rel="noreferrer">CARTO privacy policy</a></p>
	</section>
</main>

<style>
	.analytics-choice {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 12px 0;
		font-size: 15px;
	}
	.analytics-choice input {
		width: 18px;
		height: 18px;
		accent-color: var(--accent);
	}
	.preference {
		color: var(--text-secondary);
		font-size: 13px;
	}
	.privacy {
		min-height: 100dvh;
		max-width: 760px;
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
	h2 {
		font-family: var(--font-display);
		letter-spacing: 0;
	}

	h1 {
		margin: 0 0 18px;
		font-size: 34px;
	}

	h2 {
		margin: 28px 0 10px;
		font-size: 18px;
	}

	p {
		font-size: 15px;
		line-height: 1.65;
	}

	a {
		color: var(--accent);
	}

	@media (max-width: 560px) {
		.analytics-choice {
			display: flex;
			align-items: center;
			gap: 10px;
			padding: 12px 0;
			font-size: 15px;
		}
		.analytics-choice input {
			width: 18px;
			height: 18px;
			accent-color: var(--accent);
		}
		.preference {
			color: var(--text-secondary);
			font-size: 13px;
		}
		.privacy {
			padding: 24px 16px 48px;
		}

		h1 {
			font-size: 28px;
		}
	}
</style>
