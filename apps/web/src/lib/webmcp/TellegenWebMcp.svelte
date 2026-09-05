<script lang="ts">
	import { onMount } from 'svelte';
	import { getController } from '@tellegen/svelte';
	import {
		ExperimentJournal,
		registerDocumentTellegenWebMcp,
		type ExperimentRecord,
		type TellegenToolActivityEvent
	} from '@tellegen/webmcp';
	import {
		caseRevision,
		createTellegenWebMcpAdapter,
		webMcpSessionId
	} from './tellegen-adapter.js';
	import { PlanningActivityStore } from './planning-activity.svelte.js';
	import WebMcpActivity from './WebMcpActivity.svelte';

	import type { StudyWorkspace } from '../studies/workspace.svelte.js';
	import { createStudyAdapter } from '../studies/adapter.js';
	let {
		workspace,
		studyExpanded,
		closeStudy
	}: { workspace: StudyWorkspace; studyExpanded: boolean; closeStudy: () => void } = $props();
	const ctrl = getController();
	const planning = new PlanningActivityStore(() => workspace);
	const journal = new ExperimentJournal(webMcpSessionId());
	let experiments = $state.raw<ExperimentRecord[]>([]);
	let supported = $state(false);
	let registrationError = $state<string | null>(null);
	let activities = $state<TellegenToolActivityEvent[]>([]);

	function recordActivity(event: TellegenToolActivityEvent) {
		journal.record(event);
		if (event.type === 'finished') experiments = journal.records;
		const index = activities.findIndex((activity) => activity.id === event.id);
		activities =
			index === -1
				? [event, ...activities].slice(0, 12)
				: activities.map((activity, activityIndex) => (activityIndex === index ? event : activity));
	}

	function exportJournal() {
		const document = { ...journal.export(), capacityPlans: planning.entries };
		const url = URL.createObjectURL(
			new Blob([JSON.stringify(document, null, 2)], {
				type: 'application/json'
			})
		);
		const link = window.document.createElement('a');
		link.href = url;
		link.download = 'tellegen-experiments.json';
		link.click();
		setTimeout(() => URL.revokeObjectURL(url), 1_000);
	}

	// The one reactive watcher registration follows: the active case, its
	// readiness, its formulation, and its revision. Any change expires a stale
	// proposal and pulses the planning store's availability listeners.
	$effect(() => {
		const c = ctrl.activeSolvable;
		// Solving and solution readiness also control dynamic planning registration.
		void c?.solving;
		void c?.solution;
		void workspace.document?.revision;
		void workspace.document?.id;
		planning.expireIfStale(
			c && c.network && c.formulation
				? { caseId: c.id, sessionId: webMcpSessionId(), revision: caseRevision(c) }
				: null
		);
	});

	onMount(() => {
		const lifecycle = new AbortController();
		let disposed = false;
		let handle: Awaited<ReturnType<typeof registerDocumentTellegenWebMcp>> | null = null;
		const adapter = createTellegenWebMcpAdapter(ctrl, planning, workspace);
		adapter.studies = createStudyAdapter(workspace);
		void registerDocumentTellegenWebMcp(document, adapter, {
			signal: lifecycle.signal,
			onActivity: recordActivity,
			recordValidatedInput: true,
			onRegistrationError: (error) => {
				registrationError = error?.message ?? null;
				document.documentElement.dataset.webmcp = error ? 'error' : 'ready';
				if (error) console.warn('tellegen WebMCP dynamic registration failed', error);
			}
		})
			.then((registered) => {
				if (disposed) {
					registered.dispose();
					return;
				}
				handle = registered;
				supported = registered.supported;
				registrationError = registered.registrationError?.message ?? null;
				if (registered.supported) {
					document.documentElement.dataset.webmcp = registrationError ? 'error' : 'ready';
				}
			})
			.catch((error) => {
				if (!disposed) {
					registrationError = error instanceof Error ? error.message : String(error);
					document.documentElement.dataset.webmcp = 'error';
					console.warn('tellegen WebMCP registration failed', error);
				}
			});
		return () => {
			disposed = true;
			supported = false;
			registrationError = null;
			lifecycle.abort();
			handle?.dispose();
			delete document.documentElement.dataset.webmcp;
		};
	});
</script>

<WebMcpActivity
	{studyExpanded}
	{closeStudy}
	{supported}
	{registrationError}
	{activities}
	{planning}
	{experiments}
	{exportJournal}
/>
