<script lang="ts">
	import { onMount } from 'svelte';
	import { getController } from '@tellegen/svelte';
	import { registerDocumentTellegenWebMcp, type TellegenToolActivityEvent } from '@tellegen/webmcp';
	import {
		caseRevision,
		createTellegenWebMcpAdapter,
		webMcpSessionId
	} from './tellegen-adapter.js';
	import { PlanningActivityStore } from './planning-activity.svelte.js';
	import WebMcpActivity from './WebMcpActivity.svelte';

	const ctrl = getController();
	const planning = new PlanningActivityStore();
	let supported = $state(false);
	let registrationError = $state<string | null>(null);
	let activities = $state<TellegenToolActivityEvent[]>([]);

	function recordActivity(event: TellegenToolActivityEvent) {
		const index = activities.findIndex((activity) => activity.id === event.id);
		activities =
			index === -1
				? [event, ...activities].slice(0, 12)
				: activities.map((activity, activityIndex) => (activityIndex === index ? event : activity));
	}

	// The one reactive watcher registration follows: the active case, its
	// readiness, its formulation, and its revision. Any change expires a stale
	// proposal and pulses the planning store's availability listeners.
	$effect(() => {
		const c = ctrl.activeSolvable;
		// Solving and solution readiness also control dynamic planning registration.
		void c?.solving;
		void c?.solution;
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
		void registerDocumentTellegenWebMcp(document, createTellegenWebMcpAdapter(ctrl, planning), {
			signal: lifecycle.signal,
			onActivity: recordActivity,
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

<WebMcpActivity {supported} {registrationError} {activities} {planning} />
