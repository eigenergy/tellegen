<script lang="ts">
	import { createSynthSession } from '@tellegen/engine';
	import {
		getAppState,
		getController
	} from '../../../../../packages/svelte/src/lib/context.svelte.js';
	import LocalCaseDetails from '../../../../../packages/svelte/src/lib/components/LocalCaseDetails.svelte';
	const app = getAppState();
	const ctrl = getController();
	let status = $state('Ready');
	async function generate() {
		status = 'Generating';
		try {
			const session = await createSynthSession(
				{
					region: {
						geometry: {
							type: 'Polygon',
							coordinates: [
								[
									[-125, 25],
									[-68, 25],
									[-68, 49],
									[-125, 49],
									[-125, 25]
								]
							]
						},
						label: 'USA fixture'
					},
					seed: 2026,
					source: 'synthetic',
					target_bus_count: 60,
					demand: { kind: 'total_mw', mw: 1000 }
				},
				{
					places: new TextEncoder().encode(
						JSON.stringify([
							{ n: 'Los Angeles', lo: -118.2, la: 34.1, p: 4000000, c: 'USA' },
							{ n: 'Austin', lo: -97.7, la: 30.3, p: 1000000, c: 'USA' },
							{ n: 'New York', lo: -74.0, la: 40.7, p: 8000000, c: 'USA' }
						])
					)
				}
			);
			try {
				await session.run();
				await ctrl.openBalancedModule({
					name: 'Generated USA',
					payload: await session.ingest(),
					formulation: 'dcopf',
					frame: false
				});
				status = 'Generated';
			} finally {
				session.free();
			}
		} catch (error) {
			status = error instanceof Error ? error.message : String(error);
		}
	}
</script>

<button onclick={generate}>Generate USA fixture</button>
<p data-testid="generation-status">{status}</p>
<p data-testid="selected-bus">Selected bus: {app.selectedBus ?? 'none'}</p>
<LocalCaseDetails />
