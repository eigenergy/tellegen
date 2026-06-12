/** Lazy loader for the powerio wasm module. Nothing downloads until the
 * first file is dropped; the dropped file is parsed in the browser and never
 * leaves the machine. */
import type { NetworkBranch, NetworkBus } from './api';
import wasmUrl from './wasm-pkg/tellegen_wasm_bg.wasm?url';

export interface CaseFileSummary {
	name: string;
	base_mva: number;
	n_bus: number;
	n_branch: number;
	n_gen: number;
	load_mw: number;
	gen_mw: number;
	has_coords: boolean;
	warnings: string[];
}

/** One parse per dropped file: summary stats, plus map geometry when the
 * file carries coordinates. */
export interface IngestedCase extends CaseFileSummary {
	view: { buses: NetworkBus[]; branches: NetworkBranch[] } | null;
}

let ready: Promise<typeof import('./wasm-pkg/tellegen_wasm')> | null = null;

function powerio() {
	ready ??= import('./wasm-pkg/tellegen_wasm').then(async (mod) => {
		await mod.default({ module_or_path: wasmUrl });
		return mod;
	});
	return ready;
}

/** powerio format token from a file name; null for non-case files. */
export function formatOf(name: string): string | null {
	const ext = name.split('.').pop()?.toLowerCase();
	return ext === 'm' || ext === 'raw' || ext === 'aux' ? ext : null;
}

export async function ingestCase(text: string, format: string): Promise<IngestedCase> {
	return JSON.parse((await powerio()).ingest_case(text, format));
}
