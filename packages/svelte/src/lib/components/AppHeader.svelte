<script lang="ts">
	import { onDestroy, onMount, tick } from 'svelte';
	import { getAppState, getController, getUiConfig } from '../context.svelte.js';
	import { splitName } from '../format.js';
	import { getCommandRegistry, type Command } from '../commands.svelte.js';
	import { getPanelLayout } from '../panels.svelte.js';

	const app = getAppState();
	const ctrl = getController();
	const config = getUiConfig();
	const layout = getPanelLayout();
	onMount(() => {
		void ctrl.probeCollections();
	});

	let fileInput = $state.raw<HTMLInputElement | undefined>(undefined);
	function openFolder() {
		const input = document.createElement('input');
		input.type = 'file';
		input.webkitdirectory = true;
		input.multiple = true;
		input.onchange = () => {
			if (input.files) void ctrl.ingestFolder(input.files);
		};
		input.click();
	}
	// Every case is also a palette command, so a case can be opened by name.
	const commands = getCommandRegistry();
	const isMac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform);
	onDestroy(
		commands.registerSource((): Command[] => [
			// Dynamics is experimental: no commands until Settings turns it on.
			...(app.dynamicsEnabled
				? ([
						{
							id: 'dynamics:open',
							title: () => (app.mode === 'dynamics' ? 'Back to steady state' : 'Open Dynamics'),
							hint: 'RMS simulation of faults and trips',
							keywords: ['dynamics', 'rms', 'transient', 'stability', 'mode'],
							run: () => (app.mode = app.mode === 'dynamics' ? 'steady' : 'dynamics')
						},
						{
							id: 'dynamics:demo',
							title: 'Open WECC 240 dynamics demo',
							keywords: ['dynamics', 'wecc', 'demo', 'example', 'rms'],
							group: 'Cases',
							run: () =>
								void ctrl.dynamics.openDemo().catch((e) => (app.error = (e as Error).message))
						}
					] satisfies Command[])
				: []),
			...(app.dynamicsEnabled && ctrl.dynamics.active?.status === 'ready'
				? [
						{
							id: 'dynamics:run',
							title: 'Run dynamics simulation',
							keywords: ['dynamics', 'run', 'simulate'],
							run: () => void ctrl.dynamics.run()
						},
						{
							id: 'dynamics:play',
							title: () => (ctrl.dynamics.player.playing ? 'Pause playback' : 'Play dynamics'),
							keywords: ['dynamics', 'play', 'pause'],
							run: () => ctrl.dynamics.player.toggle()
						}
					]
				: []),
			{
				id: 'open-file',
				title: 'Open case file…',
				hint: 'MATPOWER, PSS/E, OpenDSS, CIM and other formats',
				keywords: ['import', 'upload', 'load'],
				group: 'Cases',
				run: () => fileInput?.click()
			},
			...app.cases
				.filter((c) => !c.unavailableReason)
				.map((c): Command => ({
					id: `case:${c.id}`,
					title: `Switch to ${c.name}`,
					group: 'Cases',
					keywords: ['case', 'network', 'open'],
					run: () => void ctrl.activateCase(c.id)
				})),
			...app.hostedDistributionCases
				.filter((c) => !c.unavailable_reason)
				.filter((c) => !app.multiCases.some((m) => m.hosted && m.id === c.id))
				.map((c): Command => ({
					id: `case:${c.id}`,
					title: `Switch to ${c.name}`,
					hint: 'Distribution case',
					group: 'Cases',
					keywords: ['case', 'network', 'open', 'distribution', 'multiconductor'],
					run: () => void ctrl.activateHostedDistribution(c.id)
				})),
			...app.localCases.map((c): Command => ({
				id: `case:${c.id}`,
				title: `Switch to ${c.label}`,
				hint: 'Opened from a file',
				group: 'Cases',
				keywords: ['case', 'network', 'open', 'local'],
				run: () => void ctrl.activateLocal(c)
			})),
			...app.multiCases.map((c): Command => ({
				id: `case:${c.id}`,
				title: `Switch to ${c.label}`,
				hint: 'Distribution case',
				group: 'Cases',
				keywords: ['case', 'network', 'open', 'distribution', 'multiconductor'],
				run: () => void ctrl.activateMulti(c)
			}))
		])
	);
	/** Publish the bar's height for the map chrome and the panels, which anchor below it. */
	function publishHeight(el: HTMLElement) {
		const observer = new ResizeObserver(() => {
			app.headerInset = Math.round(el.getBoundingClientRect().height);
		});
		observer.observe(el);
		app.headerInset = Math.round(el.getBoundingClientRect().height);
		return () => observer.disconnect();
	}

	/** The open case, named as the case list names it. */
	const current = $derived.by((): { name: string; detail: string; perturbed: boolean } | null => {
		const multi = app.activeMulti;
		if (multi) return { name: multi.label, detail: 'Distribution', perturbed: false };
		const backend = app.active;
		if (backend) {
			const [name, region] = splitName(backend.name);
			return { name, detail: region, perturbed: backend.perturbed };
		}
		const local = app.activeLocal;
		if (local) return { name: local.label, detail: 'From a file', perturbed: false };
		return null;
	});
	const hostedPending = $derived(
		app.hostedDistributionCases.filter(
			(s) => !app.multiCases.some((c) => c.hosted && c.id === s.id)
		)
	);
	const restorable = $derived(
		config.loadDefaultCases && ctrl.casesLoaded && ctrl.hiddenDefaults.size > 0
	);

	// Two menus share one pattern: a button that opens a list below it; Escape or
	// a click outside closes it and focus returns to the button.
	let caseMenuOpen = $state(false);
	let appMenuOpen = $state(false);
	let caseButton = $state.raw<HTMLButtonElement | undefined>(undefined);
	let caseList = $state.raw<HTMLElement | undefined>(undefined);
	let appButton = $state.raw<HTMLButtonElement | undefined>(undefined);
	let appList = $state.raw<HTMLElement | undefined>(undefined);

	async function toggleCaseMenu() {
		caseMenuOpen = !caseMenuOpen;
		appMenuOpen = false;
		if (caseMenuOpen) {
			await tick();
			caseList
				?.querySelector<HTMLElement>('.case-chip.active .case-activate, .case-activate')
				?.focus();
		}
	}
	async function toggleAppMenu() {
		appMenuOpen = !appMenuOpen;
		caseMenuOpen = false;
		if (appMenuOpen) {
			await tick();
			appList?.querySelector<HTMLElement>('[role="menuitem"]')?.focus();
		}
	}
	function closeMenus(returnFocus = false) {
		const button = caseMenuOpen ? caseButton : appMenuOpen ? appButton : undefined;
		caseMenuOpen = false;
		appMenuOpen = false;
		if (returnFocus) button?.focus();
	}
	/** Arrow keys move through the app menu's items, as in any menu. */
	function menuKeys(event: KeyboardEvent) {
		const items = [...(appList?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [])].filter(
			(item) => item.offsetParent !== null
		);
		const index = items.indexOf(document.activeElement as HTMLElement);
		const next =
			event.key === 'ArrowDown'
				? items[(index + 1) % items.length]
				: event.key === 'ArrowUp'
					? items[(index - 1 + items.length) % items.length]
					: event.key === 'Home'
						? items[0]
						: event.key === 'End'
							? items[items.length - 1]
							: null;
		if (next) {
			event.preventDefault();
			next.focus();
		}
	}
	function pick(run: () => unknown) {
		closeMenus();
		run();
	}
	function openSettings() {
		closeMenus();
		app.settingsOpen = true;
	}
</script>

<svelte:window
	onkeydown={(event) => {
		if (event.key === 'Escape' && (caseMenuOpen || appMenuOpen)) {
			event.preventDefault();
			closeMenus(true);
		}
	}}
	onpointerdown={(event) => {
		const target = event.target as Node;
		if (caseMenuOpen && !caseList?.contains(target) && !caseButton?.contains(target))
			caseMenuOpen = false;
		if (appMenuOpen && !appList?.contains(target) && !appButton?.contains(target))
			appMenuOpen = false;
	}}
/>

<header {@attach publishHeight}>
	<div class="brand">
		<svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
			<path class="brand-stroke" d="M4 18 L12 6 L20 18" stroke-width="1.6" fill="none" />
			<circle class="brand-node" cx="4" cy="18" r="2.4" />
			<circle class="brand-node" cx="12" cy="6" r="2.4" />
			<circle class="brand-node" cx="20" cy="18" r="2.4" />
		</svg>
		<h1>tellegen</h1>
	</div>

	<div class="case-menu">
		<button
			class="case-button"
			bind:this={caseButton}
			aria-expanded={caseMenuOpen}
			aria-controls="tellegen-case-list"
			aria-haspopup="true"
			title="Choose or open a case"
			onclick={toggleCaseMenu}
		>
			{#if current}
				<span class="case-name">{current.name}</span>
				{#if current.detail}<span class="case-detail">{current.detail}</span>{/if}
				{#if current.perturbed}<span class="case-detail">(demand changed)</span>{/if}
			{:else}
				<span class="case-name">{ctrl.casesLoaded ? 'Open a case' : 'Loading cases…'}</span>
			{/if}
			<svg class="chevron" width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"
				><path d="m3 4.5 3 3 3-3" fill="none" stroke="currentColor" stroke-width="1.5" /></svg
			>
		</button>
		<div
			class="popover case-list"
			id="tellegen-case-list"
			bind:this={caseList}
			hidden={!caseMenuOpen}
		>
			<nav aria-label="Cases">
				<ul>
					{#each app.cases as c (c.id)}
						{@const [cname, cregion] = splitName(c.name)}
						<li class="case-chip" class:active={app.activeCaseId === c.id}>
							<button
								class="case-activate"
								disabled={!!c.unavailableReason}
								title={c.unavailableReason ?? undefined}
								aria-current={app.activeCaseId === c.id ? 'true' : undefined}
								onclick={() => pick(() => ctrl.activateCase(c.id))}
							>
								<span class="cname">{cname}</span>
								<span class="cregion"
									>{c.unavailableReason ? 'Unavailable' : cregion}{c.perturbed
										? ', demand changed'
										: ''}</span
								>
							</button>
							<button
								class="case-remove"
								aria-label="Remove {c.name}"
								title="Remove {c.name}"
								onclick={(e) => ctrl.removeBackendCase(c, e)}
								><svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"
									><path d="m3 3 6 6m0-6-6 6" stroke="currentColor" stroke-width="1.4" /></svg
								></button
							>
						</li>
					{/each}
					{#each hostedPending as c (c.id)}
						<li class="case-chip">
							<button
								class="case-activate"
								disabled={!!c.unavailable_reason}
								title={c.unavailable_reason ?? undefined}
								onclick={() => pick(() => ctrl.activateHostedDistribution(c.id))}
							>
								<span class="cname">{c.name}</span>
								<span class="cregion"
									>{c.unavailable_reason
										? 'Unavailable'
										: ctrl.loadingHostedDistribution === c.id
											? 'Loading…'
											: 'Distribution'}</span
								>
							</button>
							<button
								class="case-remove"
								aria-label="Remove {c.name} from this browser"
								onclick={() => ctrl.removeHostedDistribution(c.id)}
								><svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"
									><path d="m3 3 6 6m0-6-6 6" stroke="currentColor" stroke-width="1.4" /></svg
								></button
							>
						</li>
					{/each}
					{#each app.localCases as c (c.id)}
						<li class="case-chip local" class:active={app.activeLocalId === c.id}>
							<button
								class="case-activate"
								title={c.label}
								aria-current={app.activeLocalId === c.id ? 'true' : undefined}
								onclick={() => pick(() => ctrl.activateLocal(c))}
							>
								<span class="cname">{c.label}</span>
								<span class="cregion">From a file</span>
							</button>
							<button
								class="case-remove"
								aria-label="Remove {c.label}"
								title="Remove {c.label}"
								onclick={(e) => ctrl.removeLocalCase(c, e)}
								><svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"
									><path d="m3 3 6 6m0-6-6 6" stroke="currentColor" stroke-width="1.4" /></svg
								></button
							>
						</li>
					{/each}
					{#each app.multiCases as c (c.id)}
						<li
							class={['case-chip', { local: !c.hosted, active: app.activeMultiId === c.id }]}
							data-testid="distribution-case-chip"
						>
							<button
								class="case-activate"
								aria-current={app.activeMultiId === c.id ? 'true' : undefined}
								onclick={() => pick(() => ctrl.activateMulti(c))}
							>
								<span class="cname">{c.label}</span>
								<span class="cregion">Distribution{c.hosted ? '' : ', from a file'}</span>
							</button>
							<button
								class="case-remove"
								aria-label="Remove {c.label}"
								title="Remove {c.label}"
								onclick={(e) => ctrl.removeMultiCase(c, e)}
								><svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"
									><path d="m3 3 6 6m0-6-6 6" stroke="currentColor" stroke-width="1.4" /></svg
								></button
							>
						</li>
					{/each}
				</ul>
			</nav>
			<div class="case-actions">
				<button class="action" onclick={() => pick(() => fileInput?.click())}>
					<span>Open a file…</span>
					<span class="action-hint"
						>{ctrl.collectionsAvailable
							? 'Case, ZIP or map files.'
							: 'Case or map files.'}{ctrl.showFileDropUi
							? ' You can also drop them anywhere on this page.'
							: ''}</span
					>
				</button>
				{#if ctrl.collectionsAvailable}
					<button class="action" onclick={() => pick(openFolder)}>
						<span>Open folder…</span>
						<span class="action-hint">Every case file in a shared folder.</span>
					</button>
				{/if}
				{#if restorable}
					<button class="action" onclick={() => pick(ctrl.restoreDefaultCases)}>
						<span>Restore default cases</span>
						<span class="action-hint">Bring back the demo cases you removed.</span>
					</button>
				{/if}
			</div>
		</div>
	</div>

	<div class="tools">
		{#if app.zen}
			<button
				class="tool"
				aria-label="Exit zen mode"
				title="Exit zen mode (Esc)"
				onclick={() => (app.zen = false)}>Exit zen</button
			>
		{/if}
		<button
			class="tool wide-only"
			onclick={() => commands.show()}
			aria-keyshortcuts={isMac ? 'Meta+K' : 'Control+K'}
			title="Search commands, cases and panels"
			>Commands <kbd>{isMac ? '⌘K' : 'Ctrl K'}</kbd></button
		>
		<a class="tool wide-only" href={config.docsHref} target="_blank" rel="noreferrer">Docs</a>
		<div class="app-menu">
			<button
				class="tool"
				bind:this={appButton}
				aria-expanded={appMenuOpen}
				aria-haspopup="menu"
				aria-controls="tellegen-app-menu"
				onclick={toggleAppMenu}
				>Menu<svg class="chevron" width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"
					><path d="m3 4.5 3 3 3-3" fill="none" stroke="currentColor" stroke-width="1.5" /></svg
				></button
			>
			<div
				class="popover app-list"
				id="tellegen-app-menu"
				role="menu"
				aria-label="Menu"
				tabindex="-1"
				bind:this={appList}
				hidden={!appMenuOpen}
				onkeydown={menuKeys}
			>
				<button role="menuitem" class="narrow-only" onclick={() => pick(() => commands.show())}
					>Commands</button
				>
				<button role="menuitem" onclick={openSettings}>Settings</button>
				<button role="menuitem" onclick={() => pick(() => layout.reset())}>Reset layout</button>
				<hr />
				<a
					role="menuitem"
					class="narrow-only"
					href={config.docsHref}
					target="_blank"
					rel="noreferrer"
					onclick={() => closeMenus()}>Docs</a
				>
				{#if config.showFooter}
					<a role="menuitem" href="/changelog/" onclick={() => closeMenus()}>What's new</a>
					<a role="menuitem" href="/credits/" onclick={() => closeMenus()}>Credits</a>
					<a role="menuitem" href="/privacy/" onclick={() => closeMenus()}>Privacy</a>
				{/if}
				{#if config.orgLabel}
					<a
						role="menuitem"
						class="org"
						href={config.orgHref}
						target="_blank"
						rel="noreferrer"
						onclick={() => closeMenus()}>{config.orgLabel}</a
					>
				{/if}
			</div>
		</div>
	</div>
	<input
		type="file"
		accept={`${ctrl.collectionsAvailable ? '.zip,' : ''}.m,.raw,.rawx,.aux,.epc,.pwb,.dss,.pwd,.csv,.json,.geojson,.xml,.xiidm,.uct,.ucte,.cdf,.glm,.sinx,.dyr`}
		multiple
		hidden
		bind:this={fileInput}
		onchange={(e) => {
			const input = e.currentTarget;
			// Pass the FileList through unchanged: the controller checks its declared
			// count before materializing any entries.
			if (input.files) ctrl.ingestFiles(input.files);
			input.value = '';
		}}
	/>
</header>

<style>
	/* One quiet bar: the wordmark, the case menu, and the tools. It is opaque,
	   so the map starts below it. */
	header {
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		z-index: 45;
		display: flex;
		align-items: center;
		gap: 12px;
		height: 48px;
		padding: 0 12px 0 16px;
		background: var(--panel);
		border-bottom: 1px solid var(--line);
		font-family: var(--font-sans);
	}

	.brand {
		flex: none;
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.brand-stroke {
		stroke: var(--ink);
	}

	.brand-node {
		fill: var(--accent);
	}

	h1 {
		margin: 0;
		font-size: var(--fs-title);
		font-weight: 600;
		letter-spacing: 0;
	}

	button {
		font: inherit;
		color: inherit;
		cursor: pointer;
	}

	.case-menu,
	.app-menu {
		position: relative;
		min-width: 0;
	}

	.case-menu {
		flex: 0 1 auto;
		margin-left: 8px;
	}

	.case-button {
		display: flex;
		align-items: baseline;
		gap: 8px;
		max-width: 100%;
		height: 32px;
		padding: 0 10px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		background: var(--raised);
		text-align: left;
	}

	.case-button:hover,
	.tool:hover {
		border-color: var(--border-strong);
	}

	.case-button[aria-expanded='true'] {
		border-color: var(--accent);
	}

	.case-name {
		align-self: center;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: var(--fs-body);
		font-weight: 600;
	}

	.case-detail {
		align-self: center;
		flex: none;
		color: var(--text-secondary);
		font-size: var(--fs-body);
		white-space: nowrap;
	}

	.chevron {
		align-self: center;
		flex: none;
		color: var(--text-secondary);
	}

	.popover {
		position: absolute;
		top: calc(100% + 6px);
		z-index: 50;
		background: var(--surface-menu);
		border: 1px solid var(--line);
		border-radius: var(--radius-panel);
		box-shadow: var(--elev-menu);
		animation: drop var(--dur-fast) var(--ease-out);
	}

	.popover[hidden] {
		display: none;
	}

	.case-list {
		left: 0;
		width: 320px;
		max-width: calc(100vw - 24px);
		max-height: min(70vh, 560px);
		overflow: auto;
		padding: 4px;
	}

	.case-list ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}

	/* Dense list: the condensed face fits long case names on one line. */
	.case-chip {
		position: relative;
		display: flex;
		align-items: stretch;
		border-radius: var(--radius-control);
		font-family: var(--font-condensed);
	}

	.case-chip:hover {
		background: var(--surface-hover);
	}

	.case-chip.active {
		background: var(--accent-soft);
		box-shadow: inset 2px 0 0 var(--accent);
	}

	.case-activate {
		flex: 1;
		min-width: 0;
		display: flex;
		align-items: baseline;
		gap: 8px;
		padding: 7px 8px 7px 10px;
		border: 0;
		background: none;
		text-align: left;
	}

	.case-activate:disabled {
		color: var(--text-secondary);
		cursor: not-allowed;
	}

	.cname {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: var(--fs-body);
		font-weight: 600;
	}

	.cregion {
		flex: none;
		color: var(--text-secondary);
		font-size: var(--fs-label);
		white-space: nowrap;
	}

	.case-remove {
		flex: none;
		display: grid;
		place-items: center;
		width: 32px;
		border: 0;
		border-radius: var(--radius-control);
		background: none;
		color: var(--text-tertiary);
	}

	.case-remove:hover,
	.case-remove:focus-visible {
		color: var(--text-danger);
		background: var(--surface-hover);
	}

	.case-actions {
		display: grid;
		gap: 2px;
		margin-top: 4px;
		padding-top: 4px;
		border-top: 1px solid var(--line);
	}

	.action {
		display: grid;
		gap: 2px;
		padding: 7px 10px;
		border: 0;
		border-radius: var(--radius-control);
		background: none;
		text-align: left;
		font-size: var(--fs-body);
		font-weight: 500;
	}

	.action:hover {
		background: var(--surface-hover);
	}

	.action-hint {
		color: var(--text-secondary);
		font-size: var(--fs-label);
		font-weight: 400;
	}

	.tools {
		flex: none;
		display: flex;
		align-items: center;
		gap: 4px;
		margin-left: auto;
	}

	.tool {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		height: 32px;
		padding: 0 10px;
		border: 1px solid transparent;
		border-radius: var(--radius-control);
		background: none;
		color: var(--ink);
		font-size: var(--fs-body);
		text-decoration: none;
		white-space: nowrap;
	}

	.tool[aria-expanded='true'] {
		border-color: var(--accent);
	}

	kbd {
		font: inherit;
		font-size: var(--fs-label);
		color: var(--text-secondary);
		padding: 0 4px;
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
	}

	.app-list {
		right: 0;
		min-width: 200px;
		padding: 4px;
		display: grid;
	}

	.app-list [role='menuitem'] {
		display: block;
		padding: 7px 10px;
		border: 0;
		border-radius: var(--radius-control);
		background: none;
		color: var(--ink);
		font: inherit;
		font-size: var(--fs-body);
		text-align: left;
		text-decoration: none;
	}

	.app-list [role='menuitem']:hover,
	.app-list [role='menuitem']:focus-visible {
		background: var(--surface-hover);
	}

	.app-list [role='menuitem']:focus-visible {
		outline-offset: -2px;
	}

	.app-list .org {
		color: var(--text-secondary);
		font-size: var(--fs-label);
	}

	.app-list hr {
		width: 100%;
		margin: 4px 0;
		border: 0;
		border-top: 1px solid var(--line);
	}

	.narrow-only {
		display: none !important;
	}

	@media (max-width: 760px) {
		header {
			gap: 8px;
			padding: 0 8px 0 12px;
		}

		.case-menu {
			flex: 1 1 auto;
			margin-left: 0;
		}

		.case-button .case-detail {
			display: none;
		}

		.wide-only {
			display: none;
		}

		.narrow-only {
			display: block !important;
		}
	}

	@media (hover: none), (pointer: coarse) {
		.case-activate,
		.action,
		.app-list [role='menuitem'] {
			min-height: 44px;
		}

		.case-remove {
			width: 44px;
		}

		.case-button,
		.tool {
			height: 40px;
		}
	}
</style>
