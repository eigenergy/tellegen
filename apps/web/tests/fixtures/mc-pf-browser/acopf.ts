import {
	createStudy,
	ingestCase,
	solveAcOpfModule,
	type Network,
	type Solution,
	type SolveResponse
} from '@tellegen/engine';

declare global {
	interface Window {
		prepareAcOpfInstance(text: string): Promise<string>;
		prepareBackendTransmissionCase(text: string): Promise<{
			moduleJson: string;
			network: Network;
			solution: Solution;
			nGen: number;
		}>;
		solveAcOpfCase(text: string, wasmUrl: string): Promise<SolveResponse>;
	}
}

// The server API returns the same network, cached DC result, and retained
// PowerIO module separately. Build consistent data through the real importer.
window.prepareBackendTransmissionCase = async (text) => {
	const input = await ingestCase(new TextEncoder().encode(text), 'matpower');
	const study = await createStudy(input.module_json, 'dcopf');
	try {
		const buses = input.topology.buses.map((bus, index) => ({
			...bus,
			lon: -81.1 + index * 0.1,
			lat: 34 + (index % 2) * 0.1
		}));
		return {
			moduleJson: input.module_json,
			nGen: input.n_gen,
			network: {
				id: 'transmission',
				name: 'Transmission test',
				base_mva: input.base_mva,
				synthetic_coords: true,
				buses,
				branches: input.topology.branches.map((branch) => ({
					...branch,
					path: [branch.from, branch.to].map((id) => {
						const bus = buses.find((candidate) => candidate.id === id)!;
						return [bus.lon, bus.lat] as [number, number];
					})
				}))
			},
			solution: await study.currentSolution()
		};
	} finally {
		study.free();
	}
};

// Produce a canonical instance through PowerIO's existing Study boundary,
// rather than hand-maintaining a second serializer in the browser tests.
window.prepareAcOpfInstance = async (text) => {
	const input = await ingestCase(new TextEncoder().encode(text), 'matpower');
	const study = await createStudy(input.module_json, 'socwr');
	try {
		return await study.saveInstanceModule();
	} finally {
		study.free();
	}
};

// The external corpus exercises the production importer and disposable worker.
window.solveAcOpfCase = async (text, wasmUrl) => {
	const input = await ingestCase(new TextEncoder().encode(text), 'matpower');
	return solveAcOpfModule(wasmUrl, input.module_json);
};
