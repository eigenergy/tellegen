import { createStudy, ingestCase } from '@tellegen/engine';

declare global {
	interface Window {
		prepareAcOpfInstance(text: string): Promise<string>;
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
