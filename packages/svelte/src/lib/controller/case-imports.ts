import {
	applyDisplayGeo,
	formatOf,
	ingestCase,
	ingestDistCaseBytes,
	ingestJsonDrop,
	isDisplayFile,
	parseDisplay,
	parseGeo,
	type DisplayPreview,
	type IngestedCase,
	type IngestedDistCase,
	type PowerIoDiagnostic
} from '@tellegen/engine';
import { captureDrop, namedFiles } from '../collection-files.js';
import { CollectionImport } from '../collection-import.svelte.js';
import { distExtensionFormat, isGeoFileName } from '../drop-classify.js';
import { DropBatchGate, readDropFileBytes, validateDropBatch } from '../drop-limits.js';
import { DEMO_LABEL } from '../dynamics/controller.svelte.js';
import { errorText } from '../format.js';
import { type ErrorContext } from '../log.svelte.js';
import { LocalCase, MulticonductorCase, type SolvableCase } from '../state.svelte.js';
import { placeSyntheticTopology } from '../synthetic-layout.js';

import type { Controller } from '../controller.svelte.js';
import { caseLabel, NOTICES } from '../controller.svelte.js';
type ImportHost = Pick<
	Controller,
	| 'activateLocal'
	| 'addAndActivateLocal'
	| 'app'
	| 'applyGeoLayers'
	| 'applyMultiGeoLayers'
	| 'collectionImport'
	| 'dynamics'
	| 'localNetwork'
	| 'localSeq'
	| 'probeCollections'
>;
type ImportDependencies = {
	addMultiCase: (fileName: string, payload: IngestedDistCase) => MulticonductorCase;
	attachSidecarsToSelected: (
		layers: GeoLayerFile[],
		displays?: DisplayFile[],
		expected?: SolvableCase | MulticonductorCase | null
	) => Promise<void>;
	fail: (message: string, retry?: (() => void) | null, known?: ErrorContext) => void;
	failed: (
		title: string | undefined,
		stage: string | undefined,
		cause: unknown,
		known?: Omit<ErrorContext, 'retry'> & { quiet?: boolean; retry?: (() => void) | null }
	) => void;
	localFromBalancedPayload: (fileName: string, payload: IngestedCase) => LocalCase;
	logImport: (
		fileName: string,
		summary: { diagnostics?: PowerIoDiagnostic[]; warnings?: string[] },
		moduleJson?: string,
		quiet?: boolean
	) => void;
	logPlacement: (c: LocalCase | MulticonductorCase, notes: string[], complete: boolean) => void;
	openCollection: (read: Parameters<CollectionImport['start']>[0]) => Promise<void>;
	openFailed: (name: string, cause: unknown) => void;
};
export type GeoLayerFile = {
	name: string;
	layer: string;
	diagnostics: PowerIoDiagnostic[];
};
export type DisplayFile = {
	file: File;
	bytes: Uint8Array;
	preview: DisplayPreview;
	consumed: boolean;
};
type JsonDropRoute =
	| { outcome: 'balanced'; payload: IngestedCase }
	| { outcome: 'multiconductor'; payload: IngestedDistCase }
	| { outcome: 'failed' }
	| { outcome: 'unrouted'; bytes: Uint8Array | null };
export class CaseImportActions {
	private readonly dropBatchGate = new DropBatchGate();
	constructor(
		private host: ImportHost,
		private deps: ImportDependencies
	) {}

	/** Parse dropped files in the browser via the powerio wasm module. Case
	 * files (.m, .raw, .aux, .epc, .pwb) become local networks; geographic files can
	 * place those networks; a PowerWorld .pwd becomes a substation point
	 * preview. Files run serially; nothing uploads. */
	ingestFiles = async (files: FileList | File[]) => {
		if (this.host.collectionImport.open) {
			this.host.app.error = NOTICES.collectionOpen;
			return;
		}
		const release = this.dropBatchGate.enter();
		if (!release) {
			this.host.app.error = NOTICES.batchBusy;
			return;
		}
		try {
			let list: File[];
			try {
				list = validateDropBatch(files);
			} catch (e) {
				this.deps.failed('Could not open the files', 'Open', e);
				return;
			}
			if (list.some((f) => /\.zip$/i.test(f.name) || !!f.webkitRelativePath)) {
				await this.deps.openCollection(async () => namedFiles(list));
			} else if (
				list.length > 1 &&
				list.some((f) => /\.dss$/i.test(f.name)) &&
				(await this.host.probeCollections())
			) {
				await this.deps.openCollection(async () => namedFiles(list));
			} else await this.ingestFileBatch(list);
		} catch (e) {
			this.deps.failed('Could not open the files', 'Open', e);
		} finally {
			this.host.app.parsingFile = false;
			release();
		}
	};

	/** Folder entries use collection-member limits, not the 32-upload picker cap. */
	ingestFolder = async (files: FileList | File[]) => {
		const release = this.dropBatchGate.enter();
		if (!release) {
			this.host.app.error = NOTICES.batchBusy;
			return;
		}
		try {
			await this.deps.openCollection(async () => namedFiles(files));
		} catch (error) {
			this.deps.failed('Could not open the files', 'Open', error);
		} finally {
			release();
		}
	};

	ingestDrop = (data: DataTransfer) => {
		// Capture entry handles in the drop event before its protected data expires.
		try {
			const entries = Array.from(data.items).filter((item) => item.kind === 'file');
			if (!entries.some((item) => item.webkitGetAsEntry?.()?.isDirectory)) {
				void this.ingestFiles(data.files);
				return;
			}
			const read = captureDrop(data);
			const release = this.dropBatchGate.enter();
			if (!release) {
				this.host.app.error = NOTICES.batchBusy;
				return;
			}
			void this.deps
				.openCollection(read)
				.catch((error) => {
					this.deps.failed('Could not open the files', 'Open', error);
				})
				.finally(release);
		} catch (error) {
			this.deps.failed('Could not open the files', 'Open', error);
		}
	};

	ingestFileBatch = async (all: File[]) => {
		const sidecarTarget =
			this.host.app.activeMulti ?? this.host.app.activeLocal ?? this.host.app.active;
		let parsedCaseCount = 0;

		// PSS/E dynamic data (.dyr, or a .txt that reads as DYR) attaches to
		// the balanced case of this drop, else to the active local case.
		const dyrFiles: File[] = [];
		const list: File[] = [];
		for (const file of all) {
			// With dynamics off, a .txt is never read as dynamic data.
			const dyr = this.host.app.dynamicsEnabled
				? await this.host.dynamics.isDyr(file)
				: /\.dyr$/i.test(file.name);
			if (dyr) dyrFiles.push(file);
			else list.push(file);
		}
		const localsBefore = new Set(this.host.app.localCases.map((c) => c.id));

		// Routable `.json` content (saved packages, model JSON, and transmission or
		// distribution documents) shares the extension with geographic files. Route
		// it up front, but stage recognized payloads until the geographic/display
		// sidecars below have parsed. Unrouted JSON retains its first byte buffer so
		// the geographic fallback does not read a potentially large file twice.
		const rest: File[] = [];
		const routedJson: { file: File; route: JsonDropRoute }[] = [];
		const jsonBytes = new Map<File, Uint8Array>();
		for (const file of list) {
			if (file.name.toLowerCase().endsWith('.json')) {
				const route = await this.routeJsonDrop(file);
				if (route.outcome === 'balanced' || route.outcome === 'multiconductor') {
					parsedCaseCount++;
					routedJson.push({ file, route });
					continue;
				}
				if (route.outcome === 'failed') continue;
				if (route.bytes) jsonBytes.set(file, route.bytes);
			}
			rest.push(file);
		}

		const geoLayers: GeoLayerFile[] = [];
		for (const file of rest.filter((f) => isGeoFileName(f.name))) {
			this.host.app.parsingFile = true;
			try {
				const bytes = await readDropFileBytes(file, jsonBytes.get(file));
				jsonBytes.delete(file);
				const parsed = await parseGeo(bytes, file.name);
				geoLayers.push({
					name: file.name,
					layer: parsed.layer,
					diagnostics: parsed.diagnostics
				});
				this.host.app.error = null;
			} catch (e) {
				this.deps.openFailed(file.name, e);
			} finally {
				this.host.app.parsingFile = false;
			}
		}

		// PowerWorld .pwd displays parse up front so a coordinate-less case in the
		// same drop can borrow their substation points (the SubNum join); one that
		// no case consumes becomes its own substation preview entry below.
		const displays: DisplayFile[] = [];
		for (const file of rest.filter((f) => isDisplayFile(f.name))) {
			this.host.app.parsingFile = true;
			try {
				const bytes = new Uint8Array(await file.arrayBuffer());
				displays.push({
					file,
					bytes,
					preview: await parseDisplay(bytes),
					consumed: false
				});
				this.host.app.error = null;
			} catch (e) {
				this.deps.openFailed(file.name, e);
			} finally {
				this.host.app.parsingFile = false;
			}
		}

		// Materialize routed JSON cases only after sidecars are ready. This keeps
		// placement ahead of the first solve and preserves the original JSON order.
		// `prepareDroppedLocal` is a wasm call that takes seconds on a large case,
		// and the drop gate refuses a second batch throughout, so the UI has to
		// keep saying so.
		if (routedJson.length > 0) this.host.app.parsingFile = true;
		try {
			for (const { file, route } of routedJson) {
				if (route.outcome === 'multiconductor') {
					const multi = this.deps.addMultiCase(file.name, route.payload);
					if (geoLayers.length) {
						try {
							await this.host.applyMultiGeoLayers(multi, geoLayers);
						} catch (error) {
							this.deps.failed('Could not attach coordinates', 'Coordinates', error);
						}
					}
					continue;
				}
				if (route.outcome !== 'balanced') continue;
				const local = this.deps.localFromBalancedPayload(file.name, route.payload);
				const placement = await this.prepareDroppedLocal(local, geoLayers, displays);
				this.host.addAndActivateLocal(local);
				this.host.app.error = placement.error;
			}
		} finally {
			this.host.app.parsingFile = false;
		}

		for (const file of rest.filter((f) => !isGeoFileName(f.name) && !isDisplayFile(f.name))) {
			// OpenDSS routes by extension into the multiconductor viewing path.
			const distFormat = distExtensionFormat(file.name);
			if (distFormat) {
				if (await this.ingestDistFile(file, distFormat)) {
					parsedCaseCount++;
					if (geoLayers.length && this.host.app.activeMulti) {
						try {
							await this.host.applyMultiGeoLayers(this.host.app.activeMulti, geoLayers);
						} catch (error) {
							this.deps.failed('Could not attach coordinates', 'Coordinates', error);
						}
					}
				}
				continue;
			}
			const format = formatOf(file.name);
			if (!format) {
				this.host.app.error = `${file.name}: unsupported format`;
				continue;
			}
			this.host.app.parsingFile = true;
			try {
				const bytes = new Uint8Array(await file.arrayBuffer());
				const { module_json, topology, view, ...summary } = await ingestCase(bytes, format);
				if (format === 'aux' && (summary.n_branch === 0 || summary.n_gen === 0)) {
					this.host.app.error = `${file.name}: incomplete network`;
					continue;
				}
				const id = `local-${++this.host.localSeq}`;
				// The bundled demo's RAW header is a sentence cut off at its
				// field width, not a name.
				const label =
					this.host.dynamics.attachingDemo && format === 'raw'
						? DEMO_LABEL
						: caseLabel(summary.name, file.name);
				this.deps.logImport(file.name, summary, module_json, label === DEMO_LABEL);
				const local = new LocalCase({
					id,
					label,
					fileName: file.name,
					summary,
					studyInputJson: module_json,
					topology,
					coordsKind: summary.coords_kind,
					view
				});
				const placement = await this.prepareDroppedLocal(local, geoLayers, displays);
				this.host.addAndActivateLocal(local);
				parsedCaseCount++;
				this.host.app.error = placement.error; // a successful parse clears a prior file's error
			} catch (e) {
				this.deps.openFailed(file.name, e);
			} finally {
				this.host.app.parsingFile = false;
			}
		}

		if (dyrFiles.length > 0) await this.attachDynamics(dyrFiles, localsBefore);

		if (parsedCaseCount === 0 && (geoLayers.length > 0 || displays.length > 0)) {
			await this.deps.attachSidecarsToSelected(geoLayers, displays, sidecarTarget);
		} else {
			const unmatchedDisplays = displays.filter((display) => !display.consumed);
			if (unmatchedDisplays.length)
				this.host.app.error = `${unmatchedDisplays.map((display) => display.file.name).join(' and ')}: drawing does not match case`;
		}
	};

	/** Attach dropped dynamic data: to the balanced case the same drop
	 * opened, else to the active local case. A second .dyr replaces the
	 * first after a confirm. */
	attachDynamics = async (files: File[], localsBefore: Set<string>) => {
		if (!this.host.app.dynamicsEnabled) {
			this.deps.fail(
				`${files.map((f) => f.name).join(' and ')} not opened: dynamics is off`,
				null,
				{
					title: 'Turn on Experimental features in Settings',
					stage: 'Open'
				}
			);
			return;
		}
		const added = this.host.app.localCases.filter((c) => !localsBefore.has(c.id));
		const target = added.at(-1) ?? this.host.app.activeLocal;
		if (!target || !target.studyInputJson) {
			this.host.app.error = `Drop the matching .raw case with ${files.map((f) => f.name).join(' and ')}.`;
			return;
		}
		if (files.length > 1) {
			this.host.app.error = `Drop one .dyr file at a time (${files.length} were dropped).`;
			return;
		}
		const file = files[0];
		const previous = this.host.dynamics.attachments[target.id];
		if (
			previous &&
			typeof confirm === 'function' &&
			!confirm(`Replace ${previous.fileName} with ${file.name} for ${target.label}?`)
		)
			return;
		if (this.host.app.activeLocal?.id !== target.id) this.host.activateLocal(target);
		this.host.app.mode = 'dynamics';
		await this.host.dynamics.attach(
			target,
			file.name,
			await file.text(),
			this.host.dynamics.attachingDemo
		);
	};

	/** Apply co-dropped geographic data before a local case becomes active and
	 * starts solving. Without geographic data, let a coordinate-less case borrow
	 * the first matching PowerWorld display. A failed geographic join keeps the
	 * parsed case and returns the user-facing placement error. */
	prepareDroppedLocal = async (
		local: LocalCase,
		geoLayers: GeoLayerFile[],
		displays: DisplayFile[]
	): Promise<{ geoLayersConsumed: boolean; error: string | null }> => {
		let geoLayersConsumed = false;
		let error: string | null = null;
		if (geoLayers.length > 0 && local.studyInputJson) {
			try {
				await this.host.applyGeoLayers(local, geoLayers);
				geoLayersConsumed = true;
			} catch (e) {
				error = `${geoLayers.map((layer) => layer.name).join(' + ')}: ${errorText(e)}`;
			}
		}
		if (local.studyInputJson && displays.length > 0) {
			await this.fillFromDisplaySibling(local, displays);
		}
		this.placeTopologyAsDiagram(local);
		return { geoLayersConsumed, error };
	};

	/** A case that still has no geometry gets a connectivity layout as its
	 * diagram, so it can render and solve without a placement step. */
	placeTopologyAsDiagram = (local: LocalCase) => {
		if (local.view || local.diagram || !local.topology) return;
		const view = {
			...placeSyntheticTopology(local.topology, { lon: 0, lat: 0 }),
			coordinate_space: 'diagram' as const
		};
		const layer = JSON.stringify({
			type: 'FeatureCollection',
			powerio_geo: { space: 'diagram', kind: 'synthetic' },
			features: [
				...view.buses.map((b) => ({
					type: 'Feature',
					properties: { target: 'bus', id: String(b.id) },
					geometry: { type: 'Point', coordinates: [b.lon, b.lat] }
				})),
				...view.branches.map((b) => ({
					type: 'Feature',
					properties: {
						target: 'branch',
						branch_id: String(b.id),
						from: String(b.from),
						to: String(b.to)
					},
					geometry: { type: 'LineString', coordinates: b.path }
				}))
			]
		});
		local.diagram = {
			view,
			layer,
			name: 'Network diagram',
			warnings: ['No coordinates']
		};
		local.displayMode = 'diagram';
	};

	/** Retain diagram geometry separately from the electrical case's geographic data. */
	fillFromDisplaySibling = async (c: LocalCase, displays: DisplayFile[]) => {
		if (!c.studyInputJson) return;
		for (const d of displays.filter((entry) => !entry.consumed)) {
			try {
				const payload = await applyDisplayGeo(c.studyInputJson, d.bytes);
				if (!payload.view) continue;
				c.diagram = {
					view: payload.view,
					layer: d.preview.layer,
					name: d.file.name,
					warnings: [
						`${payload.report.matched_buses} buses, ${payload.report.matched_branches} routes matched; ${payload.report.unmatched_features} unmatched objects`,
						...payload.report.notes
					]
				};
				if (!c.view) c.displayMode = 'diagram';
				c.geoWarnings = [...(c.geoWarnings ?? []), ...c.diagram.warnings];
				this.deps.logPlacement(
					c,
					c.diagram.warnings,
					!payload.report.unmatched_features && !payload.report.notes.length
				);
				d.consumed = true;
			} catch (error) {
				c.geoWarnings = [errorText(error)];
				this.deps.logPlacement(c, c.geoWarnings, false);
			}
		}
	};

	setCaseDisplayMode = (c: LocalCase, mode: 'geographic' | 'diagram') => {
		if (mode === 'diagram' ? !c.diagram : !c.view) return;
		c.displayMode = mode;
		c.network = this.host.localNetwork(c);
		this.host.app.requestFrame(c.id);
	};

	/** Route a dropped `.json` by its content. Recognized payloads return without
	 * mutating case state so co-dropped sidecars can be applied first; unrouted
	 * content returns the bytes already read for the geographic fallback. */
	routeJsonDrop = async (file: File): Promise<JsonDropRoute> => {
		let bytes: Uint8Array;
		try {
			bytes = await readDropFileBytes(file);
		} catch {
			return { outcome: 'unrouted', bytes: null };
		}
		this.host.app.parsingFile = true;
		try {
			// A multiconductor module stays in the engine; only its handle
			// comes back, never its IR text.
			const result = await ingestJsonDrop(bytes, { retain: true });
			if (result.kind === 'unknown') return { outcome: 'unrouted', bytes };
			if (result.kind === 'ambiguous') {
				this.host.app.error = `${file.name}: ambiguous format`;
				return { outcome: 'failed' };
			}
			if (result.kind === 'transmission') {
				this.host.app.error = null;
				return { outcome: 'balanced', payload: result.payload };
			}
			if (result.kind === 'module') {
				// A stored module carries either network family; the payload's own
				// marker decides the route, mirroring the engine's value kind.
				this.host.app.error = null;
				const payload = result.payload;
				if ('model' in payload) return { outcome: 'multiconductor', payload };
				return { outcome: 'balanced', payload };
			}
			if (result.kind === 'distribution') {
				this.host.app.error = null;
				return { outcome: 'multiconductor', payload: result.payload };
			}
			throw new Error('The engine returned an unsupported result for this JSON file.');
		} catch (e) {
			this.deps.openFailed(file.name, e);
			return { outcome: 'failed' };
		} finally {
			this.host.app.parsingFile = false;
		}
	};

	/** Parse a dropped OpenDSS `.dss` file into a multiconductor viewing case.
	 * Returns true when a case landed; a parse failure sets the panel error. */
	ingestDistFile = async (file: File, format: string): Promise<boolean> => {
		this.host.app.parsingFile = true;
		try {
			const bytes = new Uint8Array(await file.arrayBuffer());
			this.deps.addMultiCase(file.name, await ingestDistCaseBytes(bytes, format, { retain: true }));
			this.host.app.error = null;
			return true;
		} catch (e) {
			this.deps.openFailed(file.name, e);
			return false;
		} finally {
			this.host.app.parsingFile = false;
		}
	};

	/** Retain parsed electrical inputs and draw each declared coordinate space directly. */
}
