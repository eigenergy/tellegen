<main class="privacy">
	<a class="back mono" href="/">back to tellegen</a>
	<h1>Privacy</h1>
	<p>
		tellegen parses dropped case files in your browser. Those files are not uploaded
		to the server by the current public demo.
	</p>

	<section>
		<h2>Current Demo</h2>
		<ul>
			<li>Dropped `.m`, `.raw`, `.aux`, and `.pwd` files stay on your device.</li>
			<li>The browser uses the local file contents to draw a map preview.</li>
			<li>The server receives ordinary page and API requests for the built in demo cases.</li>
			<li>There is no analytics product wired to uploaded case contents.</li>
		</ul>
	</section>

	<section>
		<h2>Future Opt-In Sharing</h2>
		<p>
			If tellegen adds a sharing feature, it will be explicit. The action will say
			what will be sent and will require a separate confirmation before upload.
		</p>
		<p>Planned defaults for shared files:</p>
		<ul>
			<li>Raw shared files will be retained for 180 days.</li>
			<li>Derived aggregate statistics may be retained without a fixed end date.</li>
			<li>Each upload will return a share id that can be used to request deletion.</li>
			<li>Shared files will not be sold or used for third party advertising.</li>
		</ul>
	</section>

	<section>
		<h2>Hosting And Legal Notes</h2>
		<p>
			The public demo is intended to run on Hetzner infrastructure. Before accepting
			uploaded user files, the operator will conclude Hetzner's Data Processing
			Agreement and publish controller contact details.
		</p>
		<ul>
			<li><a href="https://docs.hetzner.com/general/company-and-policy/data-protection-at-hetzner/" target="_blank" rel="noreferrer">Hetzner data protection notes</a></li>
			<li><a href="https://gdpr-info.eu/art-5-gdpr/" target="_blank" rel="noreferrer">GDPR Article 5</a></li>
			<li><a href="https://gdpr-info.eu/art-6-gdpr/" target="_blank" rel="noreferrer">GDPR Article 6</a></li>
			<li><a href="https://gdpr-info.eu/art-13-gdpr/" target="_blank" rel="noreferrer">GDPR Article 13</a></li>
		</ul>
	</section>
</main>

<style>
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

	p,
	li {
		font-size: 15px;
		line-height: 1.65;
	}

	ul {
		margin: 0;
		padding-left: 20px;
	}

	a {
		color: var(--accent);
	}

	@media (max-width: 560px) {
		.privacy {
			padding: 24px 16px 48px;
		}

		h1 {
			font-size: 28px;
		}
	}
</style>
