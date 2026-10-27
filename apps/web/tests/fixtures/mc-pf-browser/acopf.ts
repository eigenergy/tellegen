import { createStudy, ingestCase, solveAcOpfModule, type SolveResponse } from '@tellegen/engine';

declare global {
	interface Window {
		prepareAcOpfInstance(text: string): Promise<string>;
		solveAcOpfCase(text: string, wasmUrl: string): Promise<SolveResponse>;
	}
}

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
