<script lang="ts">
	import { onMount } from 'svelte';
	import { ingestCase, type Network, type Solution } from '@tellegen/engine';
	import EngineeringDrawing from '../../../../../packages/svelte/src/lib/components/EngineeringDrawing.svelte';
	let network = $state<Network | null>(null);
	let error = $state('');
	let showChange = $state(false);
	let solution = $state<Solution | null>(null);
	onMount(() => {
		void (async () => {
			try {
				const query = new URLSearchParams(location.search);
				showChange = query.has('live');
				const count = Number(query.get('count'));
				if (count > 0 && count <= 10_000) {
					const columns = Math.ceil(Math.sqrt(count));
					network = {
						id: 'large',
						name: 'Large',
						base_mva: 100,
						synthetic_coords: true,
						coordinate_space: 'diagram',
						buses: Array.from({ length: count }, (_, i) => ({
							id: i,
							name: `Bus ${i}`,
							lon: (i % columns) * 160,
							lat: Math.floor(i / columns) * 160,
							demand_mw: 1,
							gen_mw: 0,
							base_kv: 110,
							area: 1,
							zone: 1,
							uid: null
						})),
						branches: Array.from({ length: count - 1 }, (_, i) => ({
							id: i,
							from: i,
							to: i + 1,
							rate_mw: 100,
							status: 1,
							path: [
								[0, 0],
								[1, 1]
							],
							uid: null
						}))
					};
					if (query.has('mesh'))
						network.branches.push(
							...network.branches
								.slice(0, Math.max(0, count - columns))
								.map((b, i) => ({ ...b, id: count + i, to: i + columns }))
						);
					solution = {
						objective: null,
						prices: [],
						vm: network.buses.map((b) => ({ bus: b.id, value: 1 })),
						va: [],
						w: [],
						dispatch: [],
						flows: network.branches.map((b) => ({ branch: b.id, mw: 10, loading: 0.1 }))
					};
					return;
				}
				const bytes = new Uint8Array(await (await fetch('/node-breaker.zip')).arrayBuffer());
				const payload = await ingestCase(bytes, 'cgmes');
				network = {
					id: 'node-breaker',
					name: 'Station',
					base_mva: payload.base_mva,
					synthetic_coords: true,
					coordinate_space: 'diagram',
					equipment: payload.topology.equipment,
					buses: payload.topology.buses.map((b, i) => ({ ...b, lon: i * 100, lat: 0 })),
					branches: payload.topology.branches.map((b) => ({
						...b,
						path: [
							[0, 0],
							[100, 0]
						]
					}))
				};
			} catch (e) {
				error = String(e);
			}
		})();
	});
</script>

{#if network}<EngineeringDrawing
		{network}
		{solution}
		label={network.name}
		onclose={() => {}}
		onselectbus={() => {}}
	/>{:else}<p role="status">{error || 'Loading station'}</p>{/if}

{#if showChange && network?.equipment?.node_breaker}
	<button
		style="position:fixed;right:8px;bottom:8px;z-index:2147483647"
		onclick={() => {
			if (!network?.equipment?.node_breaker) return;
			network = {
				...network,
				equipment: {
					...network.equipment,
					node_breaker: {
						...network.equipment.node_breaker,
						switches: network.equipment.node_breaker.switches.map((s) => ({ ...s, open: !s.open }))
					}
				}
			};
		}}
	>
		Change switch states
	</button>
{/if}
