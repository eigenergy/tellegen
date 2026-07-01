<script lang="ts">
	import { getAppState, getController, getUiConfig } from '../context.svelte.js';
	import { splitName } from '../format.js';

	const app = getAppState();
	const ctrl = getController();
	const config = getUiConfig();

	let fileInput = $state.raw<HTMLInputElement | undefined>(undefined);
</script>

<header>
	<div class="brand">
		<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
			<path d="M4 18 L12 6 L20 18" stroke="#b25e00" stroke-width="1.6" fill="none" />
			<circle cx="4" cy="18" r="2.4" fill="#b25e00" />
			<circle cx="12" cy="6" r="2.4" fill="#20242b" />
			<circle cx="20" cy="18" r="2.4" fill="#b25e00" />
		</svg>
		<h1>tellegen</h1>
	</div>
	<nav class="cases" aria-label="networks">
		{#each app.cases as c (c.id)}
			{@const [cname, cregion] = splitName(c.name)}
			<div class="case-chip" class:active={app.activeCaseId === c.id}>
				<button class="case-activate" onclick={() => ctrl.activateCase(c.id)}>
					<span class="cname"
						>{cname}{#if c.perturbed}<i class="mark" title="demand perturbed"></i>{/if}</span
					>
					<span class="cregion mono">{cregion}</span>
				</button>
				<button
					class="case-remove mono"
					aria-label="remove {c.name} from this browser"
					title="remove {c.name} from this browser"
					onclick={(e) => ctrl.removeBackendCase(c, e)}>&#10005;</button
				>
			</div>
		{/each}
		{#each app.localCases as c (c.id)}
			<div class="case-chip local" class:active={app.activeLocalId === c.id}>
				<button class="case-activate" onclick={() => ctrl.activateLocal(c)}>
					<span class="cname">{c.label}</span>
					<span class="cregion mono">local</span>
				</button>
				<button
					class="case-remove mono"
					aria-label="remove {c.label}"
					title="remove {c.label}"
					onclick={(e) => ctrl.removeLocalCase(c, e)}>&#10005;</button
				>
			</div>
		{/each}
		{#if ctrl.showFileDropUi}
			<button
				class="ghost filedrop-ui"
				title="parsed in your browser; the file never uploads"
				onclick={() => fileInput?.click()}
			>
				<span class="cname"><span class="arrow">&#8675;</span>drop a case file</span>
				<span class="cregion mono">case + geo files</span>
			</button>
		{/if}
	</nav>
	<span class="kicker mono">
		<a href={config.orgHref} target="_blank" rel="noreferrer">{config.orgLabel}</a>
		<i class="sep"></i>
		<a href={config.docsHref} target="_blank" rel="noreferrer">docs</a>
	</span>
	<input
		type="file"
		accept=".m,.raw,.aux,.pwd,.csv,.json,.geojson"
		multiple
		hidden
		bind:this={fileInput}
		onchange={(e) => {
			const input = e.currentTarget;
			if (input.files) ctrl.ingestFiles(Array.from(input.files));
			input.value = '';
		}}
	/>
</header>

<style>
	header {
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		z-index: 10;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		padding: 10px 20px;
		background: linear-gradient(rgba(236, 233, 226, 0.95), rgba(236, 233, 226, 0));
		animation: drop 0.5s ease-out both;
	}

	.brand {
		flex: 0 0 auto;
		display: flex;
		align-items: center;
		gap: 10px;
	}

	h1 {
		margin: 0;
		font-size: 22px;
		font-weight: 600;
		letter-spacing: 0;
	}

	.cases {
		flex: 1 1 auto;
		min-width: 0;
		display: flex;
		gap: 6px;
		justify-content: center;
		overflow-x: auto;
		scrollbar-width: none;
	}

	.cases::-webkit-scrollbar {
		display: none;
	}

	.cases > button,
	.case-chip {
		display: flex;
		align-items: flex-start;
		gap: 1px;
		padding: 5px 12px 4px;
		background: var(--surface-chip);
		border: 1px solid var(--line);
		border-radius: 3px;
		cursor: pointer;
		font-family: var(--font-display);
		color: var(--ink);
		transition: border-color 0.15s ease;
	}

	.cases > button {
		flex-direction: column;
	}

	.case-chip {
		align-items: stretch;
		flex: 0 0 auto;
		width: max-content;
		max-width: 156px;
		gap: 0;
		padding: 0;
		overflow: visible;
		position: relative;
	}

	.case-chip button {
		font-family: var(--font-display);
		color: inherit;
		cursor: pointer;
	}

	.case-activate {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 1px;
		min-width: 0;
		width: 100%;
		padding: 5px 20px 4px 10px;
		background: transparent;
		border: 0;
	}

	.case-remove {
		position: absolute;
		top: 1px;
		right: 1px;
		display: grid;
		place-items: center;
		width: 18px;
		height: 18px;
		padding: 0;
		background: transparent;
		border: 0;
		color: var(--text-tertiary);
		font-size: 9px;
		line-height: 1;
	}

	/* The visible button stays small; this pads the actual tap target out to
	   at least 44px. .case-chip is overflow: visible so this isn't clipped. */
	.case-remove::before {
		content: '';
		position: absolute;
		inset: -13px;
	}

	.case-remove:hover,
	.case-remove:focus-visible {
		background: var(--accent-soft);
		border-top-right-radius: 3px;
		color: var(--red);
	}

	.cases > button:hover,
	.case-chip:hover {
		border-color: var(--accent);
	}

	.case-chip.active {
		background: var(--panel);
		border-color: var(--accent);
		box-shadow: inset 0 -2px 0 var(--accent);
	}

	.cname {
		font-size: 12.5px;
		font-weight: 600;
		line-height: 1.2;
		display: inline-flex;
		align-items: center;
		gap: 5px;
	}

	.cname .mark {
		width: 5px;
		height: 5px;
		background: var(--accent-bright);
		transform: rotate(45deg);
	}

	.cregion {
		font-size: 9.5px;
		color: var(--text-secondary);
		letter-spacing: 0;
		text-transform: uppercase;
	}

	.case-chip .cregion {
		white-space: nowrap;
	}

	.case-chip .cname,
	.case-chip .cregion {
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	/* Local case chips: dashed border + graphite text, topology only. */
	.case-chip.local {
		border-style: dashed;
		color: var(--text-secondary);
	}

	.case-chip.local.active {
		background: var(--panel);
		border-color: var(--accent);
		box-shadow: inset 0 -2px 0 var(--accent);
	}

	/* Ghost chip: standing invitation to drop or pick a case file. */
	.cases > button.ghost {
		background: rgba(252, 251, 247, 0.36);
		border: 1px dashed rgba(178, 94, 0, 0.55);
		color: var(--text-secondary);
		box-shadow: inset 0 0 0 1px rgba(212, 116, 34, 0.08);
	}

	.cases > button.ghost .cregion {
		white-space: nowrap;
	}

	.cases > button.ghost:hover {
		border-color: var(--accent);
		background: var(--accent-soft);
	}

	.cases > button.ghost:hover,
	.cases > button.ghost:hover .cregion {
		color: var(--text-accent);
	}

	.kicker {
		flex: 0 0 auto;
		display: flex;
		align-items: center;
		font-size: 11px;
		text-transform: uppercase;
		letter-spacing: 0;
		color: var(--text-secondary);
		white-space: nowrap;
	}

	.kicker a {
		color: var(--text-secondary);
		text-decoration: none;
	}

	.kicker a:hover {
		color: var(--text-accent);
	}

	@media (max-width: 760px) {
		header {
			align-items: flex-start;
			flex-wrap: wrap;
			gap: 8px;
			padding: 8px 10px 12px;
			background: linear-gradient(rgba(236, 233, 226, 0.97), rgba(236, 233, 226, 0.72));
		}

		.brand {
			gap: 8px;
		}

		h1 {
			font-size: 20px;
		}

		.kicker {
			margin-left: auto;
			font-size: 9.5px;
			letter-spacing: 0;
			line-height: 2;
		}

		.cases {
			order: 3;
			width: 100%;
			overflow-x: auto;
			padding-bottom: 2px;
			scrollbar-width: none;
			scroll-padding: 10px;
			scroll-snap-type: x proximity;
			-webkit-overflow-scrolling: touch;
		}

		.cases::-webkit-scrollbar {
			display: none;
		}

		.cases > button,
		.case-chip {
			flex: 0 0 auto;
			max-width: 150px;
			min-height: 40px;
			scroll-snap-align: start;
		}

		.cases > button {
			padding: 7px 10px 6px;
		}

		.case-activate {
			padding: 7px 24px 6px 10px;
		}

		.case-remove {
			width: 22px;
			height: 22px;
		}

		.cname,
		.cregion {
			max-width: 100%;
			white-space: nowrap;
			overflow: hidden;
			text-overflow: ellipsis;
		}
	}

	@media (max-width: 420px) {
		.kicker {
			display: none;
		}

		.cases > button,
		.case-chip {
			max-width: 132px;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		header {
			animation: none;
		}
	}
</style>
