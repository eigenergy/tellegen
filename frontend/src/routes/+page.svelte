<script lang="ts" module>
	// Counter for local case ids; module level so ids stay unique across remounts.
	let localSeq = 0;
</script>

<script lang="ts">
	import { onMount } from 'svelte';
	import {
		getCaseNetworkJson,
		getCases,
		getNetwork,
		getSensitivity,
		getSolution,
		openSolveStream
	} from '$lib/api';
	import type { Network, SensitivityColumn } from '$lib/api';
	import {
		busRadius,
		lmpDomain,
		lmpGradient,
		sensFlatColor,
		sensGradient,
		sensitivityDomain,
		type RGBA
	} from '$lib/colors';
	import {
		applyGeoSidecar,
		isGeoSidecarFile,
		mergeGeoSidecars,
		parseGeoSidecar,
		type GeoSidecar
	} from '$lib/geo-sidecar';
	import { app, CaseState, type DemandRangeMode, type LocalCase } from '$lib/state.svelte';
	import { placeSyntheticTopology } from '$lib/synthetic-layout';
	import { formatOf, ingestCase, isDisplayFile, parseDisplay, solveDc } from '$lib/wasm';
	import Sparkline from '$lib/Sparkline.svelte';
	import TellegenMap from '$lib/TellegenMap.svelte';

	let abort: AbortController | null = null;
	let closeStream: (() => void) | null = null;
	let fileInput = $state.raw<HTMLInputElement | undefined>(undefined);
	let showFileDropUi = $state(true);
	let dragDepth = 0;

	const FILE_DROP_QUERY = '(hover: hover) and (pointer: fine) and (min-width: 761px)';
	type SolvableCase = CaseState | LocalCase;
	type DemandRangeAnchor = {
		caseId: string;
		bus: number;
		delta: number;
	};

	let nearbyRangeAnchor = $state<DemandRangeAnchor | null>(null);

	function isBackendCase(c: SolvableCase): c is CaseState {
		return c instanceof CaseState;
	}

	function isActiveSolveCase(c: SolvableCase): boolean {
		return isBackendCase(c) ? app.activeCaseId === c.id : app.activeLocalId === c.id;
	}

	function caseDeltas(c: SolvableCase) {
		return isBackendCase(c) ? c.deltas : (c.deltas ?? {});
	}

	function isPerturbed(c: SolvableCase | null): boolean {
		return c ? Object.values(caseDeltas(c)).some((mw) => mw !== 0) : false;
	}

	function touchLocal(c: SolvableCase) {
		if (!isBackendCase(c)) app.updateLocal(c.id, { ...c });
	}

	function setNearbyRangeAnchor(c: SolvableCase, bus: number, delta = caseDeltas(c)[bus] ?? 0) {
		nearbyRangeAnchor = { caseId: c.id, bus, delta };
	}

	function errorText(e: unknown): string {
		return e instanceof Error ? e.message : String(e);
	}

	function rgbaCss([r, g, b, a]: RGBA): string {
		return `rgba(${r}, ${g}, ${b}, ${(a / 255).toFixed(3)})`;
	}

	onMount(() => {
		const query = window.matchMedia(FILE_DROP_QUERY);
		const syncFileDropUi = () => {
			showFileDropUi = query.matches;
			if (!showFileDropUi) {
				dragDepth = 0;
				app.dragOver = false;
			}
		};
		syncFileDropUi();
		query.addEventListener('change', syncFileDropUi);
		return () => query.removeEventListener('change', syncFileDropUi);
	});

	async function load() {
		try {
			const summaries = await getCases();
			app.cases = summaries.map((s) => new CaseState(s));
			app.activeCaseId = summaries[0]?.id ?? null;
			await Promise.all(
				app.cases.map(async (c) => {
					const [network, solution] = await Promise.all([getNetwork(c.id), getSolution(c.id)]);
					c.network = network;
					c.baseSolution = solution;
					c.solution = solution;
				})
			);
			app.requestFrame('all');
		} catch (e) {
			app.error = `backend unreachable: ${e instanceof Error ? e.message : e}`;
		}
	}

	load();

	function localNetwork(c: LocalCase): Network | null {
		if (!c.summary || !c.view) return null;
		return {
			id: c.id,
			name: c.label,
			base_mva: c.summary.base_mva,
			synthetic_coords: c.coordsKind !== 'file' && c.coordsKind !== 'sidecar',
			buses: c.view.buses,
			branches: c.view.branches
		};
	}

	function withLocalSolveState(c: LocalCase): LocalCase {
		return {
			...c,
			network: localNetwork(c) ?? c.network ?? null,
			baseSolution: c.baseSolution ?? null,
			solution: c.solution ?? null,
			sensitivity: c.sensitivity ?? null,
			deltas: c.deltas ?? {},
			iterations: c.iterations ?? [],
			solving: c.solving ?? false,
			solveMs: c.solveMs ?? null,
			solveBackend: c.solveBackend ?? null,
			solveFallbackReason: c.solveFallbackReason ?? null,
			solveSeq: c.solveSeq ?? 0,
			sensitivitySeq: c.sensitivitySeq ?? 0,
			predictedObjective: c.predictedObjective ?? null
		};
	}

	function maybeStartLocalSolve(id: string) {
		const c = app.localCases.find((lc) => lc.id === id);
		if (!c?.networkJson || !c.view || !c.summary) return;
		const prepared = withLocalSolveState({ ...c, network: localNetwork(c) });
		app.updateLocal(id, prepared);
		const current = app.localCases.find((lc) => lc.id === id);
		if (current?.networkJson && current.network && !current.solution) runSolve(current, null);
	}

	function activateCase(id: string) {
		app.activeLocalId = null;
		app.placingLocalId = null;
		if (app.activeCaseId !== id) {
			clearSelection();
			app.activeCaseId = id;
		}
		app.requestFrame(id);
	}

	function activateLocal(c: LocalCase) {
		clearSelection();
		// Mirror activateCase's reset: a local and a backend case are mutually
		// exclusive, so drop the backend selection. Otherwise app.active (derived
		// from activeCaseId) stays set and its solve card keeps hovering over the
		// local view.
		app.activeCaseId = null;
		app.activeLocalId = c.id;
		app.placingLocalId = c.coordsKind === 'synthetic_pending' ? c.id : null;
		if (c.view || c.substations) app.requestFrame(c.id);
		maybeStartLocalSolve(c.id);
	}

	function addAndActivateLocal(c: LocalCase) {
		clearSelection();
		app.activeCaseId = null;
		app.addLocal(withLocalSolveState(c));
		if (c.view || c.substations) app.requestFrame(c.id);
		maybeStartLocalSolve(c.id);
	}

	function placeLocalCase(lon: number, lat: number) {
		const id = app.placingLocalId;
		const c = id ? app.localCases.find((lc) => lc.id === id) : null;
		if (!c?.topology) return;
		const view = placeSyntheticTopology(c.topology, { lon, lat });
		app.updateLocal(c.id, withLocalSolveState({
			...c,
			view,
			coordsKind: 'synthetic',
			syntheticCenter: { lon, lat }
		}));
		app.placingLocalId = null;
		app.activeLocalId = c.id;
		app.requestFrame(c.id);
		maybeStartLocalSolve(c.id);
	}

	function moveLocalCase(c: LocalCase) {
		app.activeCaseId = null;
		app.activeLocalId = c.id;
		app.placingLocalId = c.id;
	}

	function withGeoSidecar(c: LocalCase, sidecars: GeoSidecar[]): LocalCase {
		if (!c.topology || sidecars.length === 0) return c;
		const applied = applyGeoSidecar(c.topology, mergeGeoSidecars(sidecars));
		return withLocalSolveState({
			...c,
			view: applied.view,
			coordsKind: 'sidecar',
			syntheticCenter: undefined,
			geoSource: applied.sourceLabel,
			geoWarnings: [
				`${applied.matchedBuses} buses placed from ${applied.sourceLabel}`,
				...(applied.matchedBranches > 0
					? [`${applied.matchedBranches} branch paths matched from sidecar data`]
					: []),
				...applied.warnings
			]
		});
	}

	function applyGeoSidecarsToExisting(sidecars: GeoSidecar[]) {
		const target =
			(app.activeLocal?.topology ? app.activeLocal : null) ??
			app.localCases.find((c) => c.coordsKind === 'synthetic_pending') ??
			[...app.localCases].reverse().find((c) => c.topology);
		if (!target?.topology) {
			app.error = 'drop a case file with the coordinate sidecar, or select a parsed local case first';
			return;
		}
		try {
			const updated = withGeoSidecar(target, sidecars);
			app.updateLocal(target.id, updated);
			app.activeCaseId = null;
			app.activeLocalId = target.id;
			app.placingLocalId = null;
			app.requestFrame(target.id);
			maybeStartLocalSolve(target.id);
			app.error = null;
		} catch (e) {
			app.error = `${sidecars.map((s) => s.sourceNames.join(' + ')).join(' + ')}: ${
				e instanceof Error ? e.message : e
			}; use place on map for manual placement`;
		}
	}

	// Fetch and cache the raw powerio Network JSON for the browser solver.
	// Returns null when it can't be loaded, so callers fall back to the server.
	async function ensureNetworkJson(c: SolvableCase): Promise<string | null> {
		if (!isBackendCase(c)) return c.networkJson ?? null;
		if (c.networkJson) return c.networkJson;
		try {
			const json = await getCaseNetworkJson(c.id);
			c.networkJson = json;
			return json;
		} catch (e) {
			c.solveFallbackReason = `/case fetch failed: ${errorText(e)}`;
			return null;
		}
	}

	function acceptSensitivity(
		c: SolvableCase,
		col: SensitivityColumn | null,
		busId: number | null,
		sensitivitySeq?: number
	) {
		if (!col || busId === null) return;
		if (col.bus !== busId) return;
		if (!isActiveSolveCase(c) || app.selectedBus !== busId) return;
		if (sensitivitySeq !== undefined && sensitivitySeq !== (c.sensitivitySeq ?? 0)) return;
		c.sensitivity = col;
		touchLocal(c);
	}

	function finishSolve(c: SolvableCase, seq: number, sensBus: number | null) {
		if (seq !== (c.solveSeq ?? 0)) return;
		c.solving = false;
		if (isActiveSolveCase(c) && app.selectedBus === sensBus) {
			app.previewActive = false;
			app.previewDeltaMw = null;
		}
		touchLocal(c);
	}

	async function selectBus(caseId: string, busId: number) {
		app.activeLocalId = null;
		app.placingLocalId = null;
		if (app.activeCaseId !== caseId) {
			clearSelection();
			app.activeCaseId = caseId;
		}
		const c = app.byId(caseId);
		if (!c) return;
		abort?.abort();
		const ac = new AbortController();
		abort = ac;
		const sensitivitySeq = ++c.sensitivitySeq;
		app.error = null;
		app.selectedBus = busId;
		app.previewDeltaMw = null;
		app.previewActive = false;
		app.demandRangeMode = 'local';
		setNearbyRangeAnchor(c, busId);
		app.sensitivityLoading = true;
		c.sensitivity = null;
		try {
			// The dLMP/dd column from the browser solver; the server is the fallback.
			const networkJson = await ensureNetworkJson(c);
			if (networkJson) {
				const { sensitivity } = await solveDc(caseId, networkJson, c.deltas, busId);
				if (!ac.signal.aborted) acceptSensitivity(c, sensitivity, busId, sensitivitySeq);
			} else {
				const col = await getSensitivity(caseId, busId, c.deltas, ac.signal);
				if (!ac.signal.aborted) acceptSensitivity(c, col, busId, sensitivitySeq);
			}
		} catch {
			try {
				const col = await getSensitivity(caseId, busId, c.deltas, ac.signal);
				if (!ac.signal.aborted) acceptSensitivity(c, col, busId, sensitivitySeq);
			} catch (e2) {
				if (!ac.signal.aborted && !(e2 instanceof DOMException)) app.error = String(e2);
			}
		} finally {
			if (abort === ac) app.sensitivityLoading = false;
		}
	}

	async function selectLocalBus(localId: string, busId: number) {
		const c = app.localCases.find((lc) => lc.id === localId);
		if (!c?.networkJson || !c.network) return;
		app.activeCaseId = null;
		app.activeLocalId = localId;
		app.placingLocalId = null;
		abort?.abort();
		const ac = new AbortController();
		abort = ac;
		c.sensitivitySeq = (c.sensitivitySeq ?? 0) + 1;
		const sensitivitySeq = c.sensitivitySeq;
		app.error = null;
		app.selectedBus = busId;
		app.previewDeltaMw = null;
		app.previewActive = false;
		app.demandRangeMode = 'local';
		setNearbyRangeAnchor(c, busId);
		app.sensitivityLoading = true;
		c.sensitivity = null;
		touchLocal(c);
		try {
			const { sensitivity } = await solveDc(localId, c.networkJson, c.deltas ?? {}, busId);
			if (!ac.signal.aborted) acceptSensitivity(c, sensitivity, busId, sensitivitySeq);
		} catch (e) {
			if (!ac.signal.aborted) app.error = `${c.label}: ${e instanceof Error ? e.message : e}`;
		} finally {
			if (abort === ac) app.sensitivityLoading = false;
		}
	}

	function clearSelection() {
		abort?.abort();
		const c = app.active;
		if (c) {
			c.sensitivitySeq++;
			c.sensitivity = null;
		}
		const lc = app.activeLocal;
		if (lc) {
			lc.sensitivitySeq = (lc.sensitivitySeq ?? 0) + 1;
			lc.sensitivity = null;
			touchLocal(lc);
		}
		app.selectedBus = null;
		app.previewDeltaMw = null;
		app.previewActive = false;
		app.demandRangeMode = 'local';
		nearbyRangeAnchor = null;
		app.sensitivityLoading = false;
	}

	// Exact DC solve in the browser (wasm). On any failure, or when the network
	// JSON can't be fetched, reconcile via the server stream — which also shows
	// the interior point iterations.
	function runSolve(c: SolvableCase, sensBus: number | null) {
		closeStream?.();
		c.solveSeq = (c.solveSeq ?? 0) + 1;
		const seq = c.solveSeq;
		app.error = null;
		c.solving = true;
		c.solveBackend = null;
		c.solveFallbackReason = null;
		c.iterations = [];
		c.solveMs = null;
		touchLocal(c);
		ensureNetworkJson(c).then((networkJson) => {
			if (seq !== (c.solveSeq ?? 0)) return;
			if (!networkJson) {
				c.solveFallbackReason ??= 'browser network JSON unavailable';
				if (isBackendCase(c)) return serverSolve(c, sensBus, seq);
				c.solving = false;
				app.error = `${c.label}: local case has no browser network JSON`;
				touchLocal(c);
				return;
			}
			const t0 = performance.now();
			c.solveBackend = 'clarabel-wasm';
			touchLocal(c);
			solveDc(c.id, networkJson, caseDeltas(c), sensBus)
				.then(({ solution, sensitivity }) => {
					if (seq !== (c.solveSeq ?? 0)) return;
					c.solution = solution;
					if (!c.baseSolution && Object.keys(caseDeltas(c)).length === 0) c.baseSolution = solution;
					c.solveMs = Math.round(performance.now() - t0);
					acceptSensitivity(c, sensitivity, sensBus);
					finishSolve(c, seq, sensBus);
				})
				.catch((e) => {
					if (seq !== (c.solveSeq ?? 0)) return;
					c.solveFallbackReason = `browser solve failed: ${errorText(e)}`;
					if (isBackendCase(c)) serverSolve(c, sensBus, seq);
					else {
						c.solving = false;
						app.error = `${c.label}: ${c.solveFallbackReason}`;
						touchLocal(c);
					}
				});
		});
	}

	function serverSolve(c: CaseState, sensBus: number | null, seq = c.solveSeq) {
		c.solveBackend = 'ipopt-server';
		c.solveFallbackReason ??= 'browser solve unavailable';
		closeStream = openSolveStream(c.id, c.deltas, sensBus, {
			oniteration: (it) => {
				if (seq !== c.solveSeq) return;
				c.iterations = [...c.iterations, it];
			},
			onsolution: (sol) => {
				if (seq !== c.solveSeq) return;
				c.solution = sol;
				c.solveMs = sol.solve_ms;
			},
			onsensitivity: (col) => {
				if (seq !== c.solveSeq) return;
				acceptSensitivity(c, col, sensBus);
			},
			onfail: (msg) => {
				if (seq !== c.solveSeq) return;
				c.solving = false;
				app.previewActive = false;
				app.previewDeltaMw = null;
				app.error = msg;
			},
			ondone: () => {
				finishSolve(c, seq, sensBus);
			}
		});
	}

	function commitDelta(value: number) {
		const c = activeSolvable;
		const bus = app.selectedBus;
		if (!c || bus === null) return;
		c.predictedObjective = predictedDeltaObj;
		const deltas = { ...caseDeltas(c) };
		if (Math.abs(value) < 0.25) delete deltas[bus];
		else deltas[bus] = value;
		c.deltas = deltas;
		app.previewDeltaMw = value;
		app.previewActive = true;
		touchLocal(c);
		runSolve(c, bus);
	}

	function finishDemandInput(value: number) {
		if (Math.abs(value - committedDelta) < 0.25) {
			if (!activeSolvable?.solving) {
				app.previewActive = false;
				app.previewDeltaMw = null;
			}
			return;
		}
		commitDelta(value);
	}

	function resetCase(c: SolvableCase) {
		c.deltas = {};
		c.predictedObjective = null;
		app.previewDeltaMw = app.selectedBus === null ? null : 0;
		app.previewActive = app.selectedBus !== null;
		app.demandRangeMode = 'local';
		if (app.selectedBus !== null) setNearbyRangeAnchor(c, app.selectedBus, 0);
		if (c.baseSolution) c.solution = c.baseSolution;
		touchLocal(c);
		runSolve(c, app.selectedBus);
	}

	// PowerWorld .pwd files store substation symbols at diagram coordinates,
	// not lat/lon. Auto-generated TAMU layouts are Web Mercator scaled by this
	// constant with both axes in degrees: x = K·lon and y = K·mercdeg(lat),
	// where mercdeg is the Mercator ordinate expressed in degrees. So lon = x/K,
	// and latitude is the inverse gudermannian after converting y/K back to
	// radians. Hand-edited diagrams drift from this, so positions stay
	// approximate. Verified against ACTIVSg200/2000 to within ~0.02 deg.
	const PWD_MERCATOR_K = 535.81608;
	function pwdToLngLat(x: number, y: number): [number, number] {
		const lon = x / PWD_MERCATOR_K;
		const lat = (Math.atan(Math.sinh(((y / PWD_MERCATOR_K) * Math.PI) / 180)) * 180) / Math.PI;
		return [lon, lat];
	}

	/** Parse dropped files in the browser via the powerio wasm module. Case
	 * files (.m, .raw, .aux) become local networks; coordinate sidecars can
	 * place those networks; a PowerWorld .pwd becomes a substation point
	 * preview. Files run serially; nothing uploads. */
	async function ingestFiles(files: FileList | File[]) {
		const list = Array.from(files);
		const sidecars: GeoSidecar[] = [];
		for (const file of list.filter((f) => isGeoSidecarFile(f.name))) {
			app.parsingFile = true;
			try {
				sidecars.push(parseGeoSidecar(file.name, await file.text()));
				app.error = null;
			} catch (e) {
				app.error = `${file.name}: ${e instanceof Error ? e.message : e}`;
			} finally {
				app.parsingFile = false;
			}
		}

		let parsedCaseCount = 0;
		for (const file of list.filter((f) => !isGeoSidecarFile(f.name))) {
			if (isDisplayFile(file.name)) {
				app.parsingFile = true;
				try {
					const bytes = new Uint8Array(await file.arrayBuffer());
					const display = await parseDisplay(bytes);
					const points = display.substations.map((s) => {
						const [lon, lat] = pwdToLngLat(s.x, s.y);
						return { number: s.number, name: s.name, lon, lat };
					});
					const id = `local-${++localSeq}`;
					addAndActivateLocal({
						id,
						label: file.name.replace(/\.[^.]+$/, ''),
						fileName: file.name,
						summary: null,
						view: null,
						substations: { points, approximate: true }
					});
					app.error = null;
				} catch (e) {
					app.error = `${file.name}: ${e instanceof Error ? e.message : e}`;
				} finally {
					app.parsingFile = false;
				}
				continue;
			}
			const format = formatOf(file.name);
			if (!format) {
				app.error = `${file.name}: not a case or coordinate file (.m, .raw, .aux, .pwd, .csv, .json, .geojson)`;
				continue;
			}
			app.parsingFile = true;
			try {
				const text = await file.text();
				const { network_json, topology, view, ...summary } = await ingestCase(text, format);
				if (format === 'aux' && (summary.n_branch === 0 || summary.n_gen === 0)) {
					app.error = `${file.name}: aux parsed, but no complete network; drop the matching .m or .raw case file`;
					continue;
				}
				const id = `local-${++localSeq}`;
				const label =
					summary.name && summary.name !== 'case'
						? summary.name
						: file.name.replace(/\.[^.]+$/, '');
				let local: LocalCase = {
					id,
					label,
					fileName: file.name,
					summary,
					networkJson: network_json,
					topology,
					coordsKind: summary.coords_kind,
					view
				};
				if (sidecars.length > 0 && local.coordsKind === 'synthetic_pending') {
					local = withGeoSidecar(local, sidecars);
				}
				addAndActivateLocal(local);
				parsedCaseCount++;
				app.error = null; // a successful parse clears a prior file's error
			} catch (e) {
				app.error = `${file.name}: ${e instanceof Error ? e.message : e}`;
			} finally {
				app.parsingFile = false;
			}
		}

		if (sidecars.length > 0 && parsedCaseCount === 0) applyGeoSidecarsToExisting(sidecars);
	}

	function dragHasFiles(e: DragEvent): boolean {
		return showFileDropUi && (e.dataTransfer?.types.includes('Files') ?? false);
	}

	function onDragEnter(e: DragEvent) {
		if (!dragHasFiles(e)) return;
		e.preventDefault();
		dragDepth++;
		app.dragOver = true;
	}

	function onDragLeave(e: DragEvent) {
		if (!dragHasFiles(e)) return;
		dragDepth = Math.max(0, dragDepth - 1);
		if (dragDepth === 0) app.dragOver = false;
	}

	function onDragOver(e: DragEvent) {
		if (!dragHasFiles(e)) return;
		e.preventDefault();
	}

	function onDrop(e: DragEvent) {
		if (!dragHasFiles(e)) return;
		e.preventDefault();
		dragDepth = 0;
		app.dragOver = false;
		if (e.dataTransfer) ingestFiles(e.dataTransfer.files);
	}

	function splitName(name: string): [string, string] {
		const m = name.match(/^(.*?)\s*\((.*)\)$/);
		return m ? [m[1], m[2]] : [name, ''];
	}

	const activeSolvable = $derived.by(
		(): SolvableCase | null => app.active ?? (app.activeLocal?.network ? app.activeLocal : null)
	);

	const stats = $derived.by(() => {
		const c = activeSolvable;
		if (!c?.network || !c.solution || !c.baseSolution) return null;
		const lmps = c.solution.lmp.map((e) => e.usd_per_mwh);
		const domain = lmpDomain(lmps);
		const lmpMin = Math.min(...lmps);
		const lmpMax = Math.max(...lmps);
		return {
			buses: c.network.buses.length,
			branches: c.network.branches.length,
			objective: c.solution.objective,
			deltaObjective: c.solution.objective - c.baseSolution.objective,
			uniformLmp: lmpMax - lmpMin < 1 ? lmps[0] : null,
			// Mark legend ends when outliers clamp beyond the trimmed domain.
			lmpLo: { value: domain.lo, clamped: lmpMin < domain.lo - 0.05 },
			lmpHi: { value: domain.hi, clamped: lmpMax > domain.hi + 0.05 },
			binding: c.solution.flows.filter((f) => f.loading >= 0.999).length
		};
	});

	const selectedBusData = $derived.by(() => {
		const c = activeSolvable;
		if (!c?.network || app.selectedBus === null) return null;
		return c.network.buses.find((b) => b.id === app.selectedBus) ?? null;
	});

	const committedDelta = $derived(
		activeSolvable && app.selectedBus !== null ? (caseDeltas(activeSolvable)[app.selectedBus] ?? 0) : 0
	);
	const sliderValue = $derived(app.previewDeltaMw ?? committedDelta);
	const nearbyRangeCenter = $derived.by(() => {
		const c = activeSolvable;
		const bus = app.selectedBus;
		if (!c || bus === null) return committedDelta;
		if (nearbyRangeAnchor?.caseId === c.id && nearbyRangeAnchor.bus === bus) {
			return nearbyRangeAnchor.delta;
		}
		return committedDelta;
	});

	function demandBounds(
		mode: DemandRangeMode,
		bus: typeof selectedBusData,
		center: number
	): { min: number; max: number; span: number } {
		if (!bus) return { min: 0, max: 0, span: 0 };
		const physicalMin = -Math.ceil(bus.demand_mw);
		const physicalMax = Math.max(Math.ceil(bus.demand_mw), 50);
		if (mode === 'full') return { min: physicalMin, max: physicalMax, span: physicalMax - physicalMin };
		const span = Math.max(5, Math.min(25, 0.1 * Math.max(bus.demand_mw, 50)));
		return {
			min: Math.max(physicalMin, center - span),
			max: Math.min(physicalMax, center + span),
			span
		};
	}

	const sliderBounds = $derived(demandBounds(app.demandRangeMode, selectedBusData, nearbyRangeCenter));
	const sliderMin = $derived(sliderBounds.min);
	const sliderMax = $derived(sliderBounds.max);

	function setDemandRangeMode(mode: DemandRangeMode) {
		app.demandRangeMode = mode;
		const c = activeSolvable;
		if (mode === 'local' && c && app.selectedBus !== null) {
			setNearbyRangeAnchor(c, app.selectedBus, sliderValue);
		}
		const bounds = demandBounds(mode, selectedBusData, mode === 'local' ? sliderValue : nearbyRangeCenter);
		if (app.previewDeltaMw === null) return;
		app.previewDeltaMw = Math.min(bounds.max, Math.max(bounds.min, app.previewDeltaMw));
	}

	const selectedSensitivity = $derived.by(() => {
		const c = activeSolvable;
		if (!c?.sensitivity || app.selectedBus === null) return null;
		return c.sensitivity.bus === app.selectedBus ? c.sensitivity : null;
	});

	const sensSummary = $derived.by(() =>
		selectedSensitivity ? sensitivityDomain(selectedSensitivity.values.map((v) => v.value)) : null
	);
	const flatSensBackground = $derived(sensSummary ? rgbaCss(sensFlatColor(sensSummary)) : '');

	const selectedLmp = $derived.by(() => {
		const c = activeSolvable;
		if (!c?.solution || app.selectedBus === null) return null;
		return c.solution.lmp.find((e) => e.bus === app.selectedBus)?.usd_per_mwh ?? null;
	});

	const selfSens = $derived.by(() => {
		if (!selectedSensitivity || app.selectedBus === null) return 0;
		return selectedSensitivity.values.find((v) => v.bus === app.selectedBus)?.value ?? 0;
	});

	// Second order objective preview vs base: the exact part up to the
	// committed point, plus lmp*step + S_bb*step^2/2 along the gradient.
	const predictedDeltaObj = $derived.by(() => {
		const c = activeSolvable;
		if (!c?.solution || !c.baseSolution || selectedLmp === null) return null;
		const step = sliderValue - committedDelta;
		const committedPart = c.solution.objective - c.baseSolution.objective;
		return committedPart + selectedLmp * step + 0.5 * selfSens * step * step;
	});

	const gradientScore = $derived.by(() => {
		const c = activeSolvable;
		if (!c?.solution || !c.baseSolution || c.predictedObjective == null || c.solving)
			return null;
		const exact = c.solution.objective - c.baseSolution.objective;
		return { pred: c.predictedObjective, exact };
	});

	const topMovers = $derived.by(() => {
		if (!selectedSensitivity || sensSummary?.flat) return [];
		return [...selectedSensitivity.values]
			.filter((v) => v.bus !== app.selectedBus)
			.sort((a, b) => Math.abs(b.value) - Math.abs(a.value))
			.slice(0, 5);
	});
	const showMoverSlot = $derived(Boolean(selectedSensitivity && !sensSummary?.flat));

	const previewing = $derived(
		Boolean(
			activeSolvable?.solving ||
				app.previewActive ||
				(app.previewDeltaMw !== null && Math.abs(sliderValue - committedDelta) >= 0.25)
		)
	);

	const fmt = new Intl.NumberFormat('en-US', { maximumFractionDigits: 1 });
	const signed = (v: number) => `${v < 0 ? '−' : '+'}${fmt.format(Math.abs(v))}`;
	const signedExp = (v: number) => `${v < 0 ? '−' : '+'}${Math.abs(v).toExponential(2)}`;
	const SIZE_SAMPLES = [10, 100, 500];

	function sliderCurrent() {
		return sliderValue;
	}

	function setSliderPreview(value: number | undefined) {
		if (value === undefined) return;
		app.previewActive = true;
		app.previewDeltaMw = value;
	}

	function solveBackendLabel(c: SolvableCase): string {
		if (c.solveBackend === 'clarabel-wasm') return 'Clarabel wasm';
		if (c.solveBackend === 'ipopt-server') return 'server fallback';
		return c.solving ? 'starting' : 'solve pending';
	}

	function solveMetaLabel(c: SolvableCase): string {
		if ((c.iterations ?? []).length > 1) return `${c.iterations?.length} iterations`;
		return c.solveBackend === 'ipopt-server' ? 'server solve' : 'browser solve';
	}
</script>

<svelte:window
	onkeydown={(e) => {
		if (e.key === 'Escape') clearSelection();
	}}
	ondragenter={onDragEnter}
	ondragleave={onDragLeave}
	ondragover={onDragOver}
	ondrop={onDrop}
/>

<main>
	<TellegenMap
		onbusclick={selectBus}
		onlocalbusclick={selectLocalBus}
		onplacecase={placeLocalCase}
		onmapclick={clearSelection}
	/>

	<header>
		<div class="brand">
			<svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
				<path d="M4 18 L12 6 L20 18" stroke="#b25e00" stroke-width="1.6" fill="none" />
				<circle cx="4" cy="18" r="2.4" fill="#b25e00" />
				<circle cx="12" cy="6" r="2.4" fill="#20242b" />
				<circle cx="20" cy="18" r="2.4" fill="#b25e00" />
			</svg>
			<h1>tellegen</h1>
		</div>
		<nav class="cases" aria-label="networks">
			{#each app.cases as c (c.id)}
				{@const [cname, cregion] = splitName(c.name)}
				<button class:active={app.activeCaseId === c.id} onclick={() => activateCase(c.id)}>
					<span class="cname">{cname}{#if c.perturbed}<i class="mark" title="demand perturbed"
							></i>{/if}</span>
					<span class="cregion mono">{cregion}</span>
				</button>
			{/each}
			{#each app.localCases as c (c.id)}
				<div class="case-chip local" class:active={app.activeLocalId === c.id}>
					<button class="local-activate" onclick={() => activateLocal(c)}>
						<span class="cname">{c.label}</span>
						<span class="cregion mono">local</span>
					</button>
					<button
						class="local-remove mono"
						aria-label="remove {c.label}"
						title="remove {c.label}"
						onclick={() => app.removeLocal(c.id)}>&#10005;</button
					>
				</div>
			{/each}
			{#if showFileDropUi}
				<button
					class="ghost filedrop-ui"
					title="parsed in your browser; the file never uploads"
					onclick={() => fileInput?.click()}
				>
					<span class="cname"><span class="arrow">&#8675;</span>drop a case file</span>
					<span class="cregion mono">case + geo sidecars &mdash; or click</span>
				</button>
			{/if}
		</nav>
		<span class="kicker mono">differentiable power systems</span>
	</header>

	<aside class="panel">
		{#if app.error}
			<p class="error mono">{app.error}</p>
		{/if}
		{#if app.parsingFile}
			<p class="dim mono blink">parsing&hellip;</p>
		{/if}
		{#if app.activeLocal}
			{@const lc = app.activeLocal}
			<h2>{lc.label} <span class="region mono">via {lc.fileName}</span></h2>
			{#if lc.substations}
				<dl class="mono">
					<div><dt>substations</dt><dd>{lc.substations.points.length}</dd></div>
				</dl>
				<p class="footnote mono">
					display only &mdash; positions inferred from the PowerWorld diagram, not surveyed
					latitude and longitude
				</p>
				<p class="footnote mono">decoded in your browser by powerio (wasm); never uploaded</p>
			{:else if lc.summary}
				<dl class="mono">
					<div><dt>buses</dt><dd>{lc.summary.n_bus}</dd></div>
					<div><dt>branches</dt><dd>{lc.summary.n_branch}</dd></div>
					<div><dt>generators</dt><dd>{lc.summary.n_gen}</dd></div>
					<div><dt>load</dt><dd>{fmt.format(lc.summary.load_mw)} MW</dd></div>
					<div><dt>gen capacity</dt><dd>{fmt.format(lc.summary.gen_mw)} MW</dd></div>
					<div><dt>base MVA</dt><dd>{fmt.format(lc.summary.base_mva)}</dd></div>
				</dl>
				{#if lc.summary.warnings.length > 0}
					<ul class="warnings mono">
						{#each lc.summary.warnings.slice(0, 4) as w, i (i)}
							<li>{w}</li>
						{/each}
						{#if lc.summary.warnings.length > 4}
							<li>+{lc.summary.warnings.length - 4} more</li>
						{/if}
					</ul>
				{/if}
				{#if !lc.view}
					<p class="footnote mono">
						no coordinates in this file &mdash; click the map or drop a coordinate sidecar
					</p>
				{:else if lc.coordsKind === 'synthetic'}
					<p class="footnote mono">
						coordinates: synthetic topology layout centered where you placed it
					</p>
				{:else if lc.coordsKind === 'sidecar'}
					<p class="footnote mono">
						coordinates: uploaded sidecar data from {lc.geoSource}
					</p>
				{/if}
				{#if lc.geoWarnings && lc.geoWarnings.length > 0}
					<ul class="warnings mono">
						{#each lc.geoWarnings.slice(0, 4) as w, i (i)}
							<li>{w}</li>
						{/each}
						{#if lc.geoWarnings.length > 4}
							<li>+{lc.geoWarnings.length - 4} more</li>
						{/if}
					</ul>
				{/if}
				<p class="footnote mono">
					parsed in your browser by powerio (wasm); never uploaded
				</p>
			{/if}
			{#if lc.topology && lc.coordsKind !== 'file'}
				<button class="reset mono" onclick={() => moveLocalCase(lc)}>
					{lc.coordsKind === 'synthetic_pending'
						? 'place on map'
						: lc.coordsKind === 'sidecar'
							? 'place manually'
							: 'move layout'}
				</button>
			{/if}
			<button class="reset mono" onclick={() => app.removeLocal(lc.id)}>remove</button>
		{/if}
		{#if !stats}
			{#if !app.error && !app.activeLocal}
				<p class="dim mono blink">loading cases&hellip;</p>
			{/if}
		{:else}
			{#if !app.activeLocal}
				{@const [cname, cregion] = splitName(app.active?.name ?? '')}
				<h2>{cname} <span class="region mono">{cregion}</span></h2>
				<dl class="mono">
					<div><dt>buses</dt><dd>{stats.buses}</dd></div>
					<div><dt>branches</dt><dd>{stats.branches}</dd></div>
					<div><dt>binding lines</dt><dd>{stats.binding}</dd></div>
					<div><dt>objective</dt><dd>{fmt.format(stats.objective)} $/h</dd></div>
					{#if isPerturbed(activeSolvable)}
						<div class="delta"><dt>vs base</dt><dd>{signed(stats.deltaObjective)} $/h</dd></div>
					{/if}
				</dl>
				{#if app.active?.network?.synthetic_coords}
					<p class="footnote mono">coordinates: topology layout, not geography</p>
				{:else}
					<p class="footnote mono">coordinates: TAMU synthetic grid footprint</p>
				{/if}
			{/if}

			<hr />

			{#if app.selectedBus !== null && selectedSensitivity}
				{@const c = activeSolvable as SolvableCase}
				<div class="mode">
					<span class="chip">{previewing ? 'LMP preview' : '∂LMP/∂d'}</span>
					<span class="mono dim">bus {app.selectedBus}</span>
					<button class="mono" onclick={clearSelection}>esc&nbsp;clear</button>
				</div>
				<div class="sensitivity-readout" aria-live="polite">
					{#if previewing}
						<p class="dim small">
							{c.solving
								? 'Exact solve running; the map stays in LMP view.'
								: 'First order LMP preview. Release for the exact solve.'}
						</p>
					{:else}
						<p class="dim small">Price response per MW of demand added at bus {app.selectedBus}.</p>
						{#if sensSummary?.flat}
							<div class="legend flat" style:background={flatSensBackground}></div>
							<div class="legend-labels mono single">
								<span>uniform {signedExp(sensSummary.mean)} ($/MWh)/MW</span>
							</div>
						{:else if sensSummary}
							<div class="legend" style:background={sensGradient}></div>
							<div class="legend-labels mono">
								<span>&minus;{sensSummary.scale.toExponential(1)}</span>
								<span>0</span>
								<span>+{sensSummary.scale.toExponential(1)}</span>
							</div>
						{/if}
					{/if}
				</div>

				<div class="slider-block">
					<div class="slider-head mono">
						<span>&Delta; demand</span>
						<span class="val">{signed(sliderValue)} MW</span>
					</div>
					<div class="range-mode">
						<div class="segment mono" aria-label="demand range">
							<button
								type="button"
								class:active={app.demandRangeMode === 'local'}
								aria-pressed={app.demandRangeMode === 'local'}
								aria-label="nearby demand range"
								title="range near the selected demand setting"
								onclick={() => setDemandRangeMode('local')}>nearby</button
							>
							<button
								type="button"
								class:active={app.demandRangeMode === 'full'}
								aria-pressed={app.demandRangeMode === 'full'}
								aria-label="full demand range"
								title="range from zero load to the local physical limit"
								onclick={() => setDemandRangeMode('full')}>full range</button
							>
						</div>
						<span class="mono dim">{fmt.format(sliderMin)} to {fmt.format(sliderMax)} MW</span>
					</div>
					<input
						type="range"
						min={sliderMin}
						max={sliderMax}
						step="0.5"
						bind:value={sliderCurrent, setSliderPreview}
						aria-label="demand delta at selected bus"
						onpointerdown={() => {
							app.previewActive = true;
							app.previewDeltaMw = sliderValue;
						}}
						onkeydown={() => {
							app.previewActive = true;
							app.previewDeltaMw = sliderValue;
						}}
						onpointerup={(e) => finishDemandInput(Number(e.currentTarget.value))}
						onmouseup={(e) => finishDemandInput(Number(e.currentTarget.value))}
						onclick={(e) => finishDemandInput(Number(e.currentTarget.value))}
						onkeyup={(e) => finishDemandInput(Number(e.currentTarget.value))}
						onblur={(e) => finishDemandInput(Number(e.currentTarget.value))}
						onchange={(e) => finishDemandInput(Number(e.currentTarget.value))}
					/>
					<div class="demand-feedback">
						<p class="pred mono dim" aria-hidden={!(predictedDeltaObj !== null && previewing)}>
							{#if predictedDeltaObj !== null && previewing}
								predicted &Delta;cost {signed(predictedDeltaObj)} $/h
							{:else}
								&nbsp;
							{/if}
						</p>
						<p class="score mono" aria-hidden={!(gradientScore && isPerturbed(c))}>
							{#if gradientScore && isPerturbed(c)}
								gradient {signed(gradientScore.pred)} &middot; exact {signed(gradientScore.exact)}
								$/h
							{:else}
								&nbsp;
							{/if}
						</p>
						<div class="reset-row">
							{#if isPerturbed(c)}
								<button class="reset mono" onclick={() => resetCase(c)}>reset demand</button>
							{/if}
						</div>
					</div>
				</div>

				{#if showMoverSlot}
					<div class="movers-block">
						{#if !previewing && topMovers.length > 0}
							<table class="mono">
								<tbody>
									{#each topMovers as mover (mover.bus)}
										<tr>
											<td>bus {mover.bus}</td>
											<td class:pos={mover.value > 0} class:neg={mover.value < 0}>
												{mover.value >= 0 ? '+' : ''}{mover.value.toExponential(2)}
											</td>
										</tr>
									{/each}
								</tbody>
							</table>
						{/if}
					</div>
				{/if}
				{:else}
					<div class="mode">
						<span class="chip">LMP</span>
						<span class="mono dim">$/MWh</span>
						{#if app.sensitivityLoading}
							<span class="mono dim blink">&part; loading&hellip;</span>
						{/if}
					</div>
					<p class="dim small">
						DC OPF prices. Select a bus for &part;LMP/&part;d and demand perturbation.
					</p>
					<div class="legend" style:background={lmpGradient}></div>
					<div class="legend-labels mono">
						{#if stats.uniformLmp !== null}
							<span>uniform {fmt.format(stats.uniformLmp)} $/MWh, no congestion</span>
						{:else}
							<span>{stats.lmpLo.clamped ? '≤' : ''}{fmt.format(stats.lmpLo.value)}</span>
							<span>{stats.lmpHi.clamped ? '≥' : ''}{fmt.format(stats.lmpHi.value)}</span>
						{/if}
					</div>
					<p class="dim small filedrop-note">
						Drop case files and optional coordinate sidecars; parsing stays in your browser.
					</p>
			{/if}

			<hr />

			<div class="sizes">
				{#each SIZE_SAMPLES as mw (mw)}
					<span class="size mono">
						<i style:width="{2 * busRadius(mw)}px" style:height="{2 * busRadius(mw)}px"></i>
						{mw}
					</span>
				{/each}
				<span class="mono dim caption">MW, max(load,&#8201;gen)</span>
			</div>
		{/if}
	</aside>

	{#if activeSolvable && (activeSolvable.solving || activeSolvable.solveMs != null)}
		<div class="solvecard">
			<div class="solvecard-head mono">
				<span>exact solve</span>
				{#if activeSolvable.solving}
					<span class="dim blink"
						>{solveBackendLabel(activeSolvable)}</span
					>
				{:else}
					<span class="dim"
						>{solveBackendLabel(activeSolvable)}</span
					>
				{/if}
			</div>
			{#if (activeSolvable.iterations ?? []).length > 1}
				<Sparkline iterations={activeSolvable.iterations ?? []} />
			{/if}
			<div class="solve-meta mono dim">
				<span>{solveMetaLabel(activeSolvable)}</span>
				{#if activeSolvable.solveMs != null}<span>{activeSolvable.solveMs} ms</span>{/if}
			</div>
			{#if activeSolvable.solveBackend === 'ipopt-server' && activeSolvable.solveFallbackReason}
				<p class="fallback-reason mono dim" title={activeSolvable.solveFallbackReason}>
					fallback: {activeSolvable.solveFallbackReason}
				</p>
			{/if}
		</div>
	{/if}

	{#if app.dragOver}
		<div class="dropzone" aria-hidden="true">
			<div class="dropframe">
				<p class="mono">drop to parse &mdash; case files or coordinate sidecars</p>
				<p class="mono hint">parsed in your browser; the file never uploads</p>
			</div>
		</div>
	{/if}

	{#if app.placingLocalId}
		<div class="placement-cue mono">
			click the map to place the synthetic topology
		</div>
	{/if}

	<footer class="mono">
		<a href="https://electricgrids.engr.tamu.edu/" target="_blank" rel="noreferrer"
			>ACTIVSg synthetic grids</a
		>
		<i class="sep"></i>
		<a href="https://github.com/eigenergy/powerio" target="_blank" rel="noreferrer"
			>powerio parser</a
		>
		<i class="sep"></i>
		<a href="https://github.com/eigenergy/tellegen" target="_blank" rel="noreferrer"
			>tellegen framework</a
		>
		<i class="sep"></i>
		<a href="/privacy">privacy</a>
		{#if showFileDropUi}
			<i class="sep filedrop-ui"></i>
			<span class="drophint filedrop-ui"
				><span class="arrow">&#8675;</span> drop a case or coordinate file anywhere</span
			>
		{/if}
	</footer>

	{#if showFileDropUi}
		<input
			type="file"
			accept=".m,.raw,.aux,.pwd,.csv,.json,.geojson"
			multiple
			hidden
			bind:this={fileInput}
			onchange={(e) => {
				const input = e.currentTarget;
				if (input.files) ingestFiles(Array.from(input.files));
				input.value = '';
			}}
		/>
	{/if}
</main>

<style>
	main {
		position: fixed;
		inset: 0;
		overflow: hidden;
	}

	header {
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		z-index: 10;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		padding: 10px 20px;
		background: linear-gradient(rgba(236, 233, 226, 0.95), rgba(236, 233, 226, 0));
		animation: drop 0.5s ease-out both;
	}

	.brand {
		display: flex;
		align-items: center;
		gap: 10px;
	}

	h1 {
		margin: 0;
		font-size: 22px;
		font-weight: 600;
		letter-spacing: 0;
	}

	.cases {
		display: flex;
		gap: 6px;
	}

	.cases > button,
	.case-chip {
		display: flex;
		align-items: flex-start;
		gap: 1px;
		padding: 5px 12px 4px;
		background: rgba(252, 251, 247, 0.65);
		border: 1px solid var(--line);
		border-radius: 3px;
		cursor: pointer;
		font-family: var(--font-display);
		color: var(--ink);
		transition: border-color 0.15s ease;
	}

	.cases > button {
		flex-direction: column;
	}

	.case-chip {
		align-items: stretch;
		gap: 0;
		padding: 0;
		overflow: hidden;
	}

	.case-chip button {
		font-family: var(--font-display);
		color: inherit;
		cursor: pointer;
	}

	.local-activate {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 1px;
		min-width: 0;
		padding: 5px 8px 4px 12px;
		background: transparent;
		border: 0;
	}

	.local-remove {
		align-self: stretch;
		padding: 0 7px;
		background: transparent;
		border: 0;
		border-left: 1px solid var(--line);
		color: var(--ink-faint);
		font-size: 9px;
	}

	.cases > button:hover,
	.case-chip:hover {
		border-color: var(--accent);
	}

	.cases > button.active,
	.case-chip.active {
		background: var(--panel);
		border-color: var(--accent);
		box-shadow: inset 0 -2px 0 var(--accent);
	}

	.cname {
		font-size: 12.5px;
		font-weight: 600;
		line-height: 1.2;
		display: inline-flex;
		align-items: center;
		gap: 5px;
	}

	.cname .mark {
		width: 5px;
		height: 5px;
		background: var(--accent-bright);
		transform: rotate(45deg);
	}

	.cregion {
		font-size: 9.5px;
		color: var(--ink-dim);
		letter-spacing: 0;
		text-transform: uppercase;
	}

	/* Local case chips: dashed border + graphite text, topology only. */
	.case-chip.local {
		border-style: dashed;
		color: var(--ink-dim);
	}

	.case-chip.local.active {
		background: var(--panel);
		border-color: var(--accent);
		box-shadow: inset 0 -2px 0 var(--accent);
	}

	/* Ghost chip: standing invitation to drop or pick a case file. */
	.cases > button.ghost {
		background: transparent;
		border: 1px dashed var(--ink-faint);
		color: var(--ink-dim);
	}

	.cases > button.ghost:hover {
		border-color: var(--accent);
		background: var(--accent-soft);
	}

	.cases > button.ghost:hover,
	.cases > button.ghost:hover .cregion {
		color: var(--accent);
	}

	.arrow {
		display: inline-block;
		animation: bob 1.8s ease-in-out infinite alternate;
	}

	.local-remove:hover {
		color: var(--red);
	}

	.kicker {
		font-size: 11px;
		text-transform: uppercase;
		letter-spacing: 0;
		color: var(--ink-dim);
	}

	.panel {
		position: absolute;
		top: 64px;
		left: 20px;
		z-index: 10;
		width: 312px;
		max-height: calc(100% - 110px);
		overflow-y: auto;
		padding: 18px 20px;
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: 3px;
		backdrop-filter: blur(6px);
		box-shadow: 0 4px 24px rgba(32, 36, 43, 0.08);
		animation: rise 0.5s 0.12s ease-out both;
	}

	h2 {
		margin: 0 0 12px;
		font-size: 16px;
		font-weight: 600;
	}

	.region {
		font-size: 10px;
		font-weight: 400;
		color: var(--ink-dim);
		text-transform: uppercase;
		letter-spacing: 0;
		margin-left: 4px;
	}

	dl {
		margin: 0;
		font-size: 12.5px;
	}

	dl div {
		display: flex;
		justify-content: space-between;
		padding: 3px 0;
	}

	dl .delta dd {
		color: var(--accent);
	}

	dt {
		color: var(--ink-dim);
	}

	dd {
		margin: 0;
	}

	.footnote {
		margin: 8px 0 0;
		font-size: 10px;
		color: var(--ink-faint);
		letter-spacing: 0;
	}

	.warnings {
		margin: 8px 0 0;
		padding: 0;
		list-style: none;
		font-size: 10.5px;
		line-height: 1.5;
		color: var(--accent);
	}

	hr {
		border: 0;
		border-top: 1px solid var(--line);
		margin: 14px 0;
	}

	.mode {
		display: flex;
		align-items: center;
		gap: 10px;
		font-size: 12px;
	}

	.chip {
		font-family: var(--font-mono);
		font-size: 11px;
		padding: 2px 8px;
		border: 1px solid var(--accent);
		color: var(--accent);
		background: var(--accent-soft);
		border-radius: 2px;
		white-space: nowrap;
	}

	.mode button {
		margin-left: auto;
		font-size: 10.5px;
		padding: 2px 7px;
		background: none;
		border: 1px solid var(--line);
		border-radius: 2px;
		color: var(--ink-dim);
		cursor: pointer;
	}

	.mode button:hover {
		border-color: var(--accent);
		color: var(--accent);
	}

	.small {
		font-size: 12px;
		line-height: 1.55;
	}

	.dim {
		color: var(--ink-dim);
	}

	.error {
		color: var(--red);
		font-size: 12px;
	}

	.sensitivity-readout {
		min-height: 78px;
	}

	.legend {
		height: 6px;
		border-radius: 3px;
		margin-top: 6px;
	}

	.legend-labels {
		display: flex;
		justify-content: space-between;
		font-size: 10.5px;
		color: var(--ink-faint);
		margin-top: 4px;
	}

	.legend-labels.single {
		justify-content: center;
		text-align: center;
	}

	.slider-block {
		margin-top: 14px;
	}

	.slider-head {
		display: flex;
		justify-content: space-between;
		font-size: 11.5px;
		color: var(--ink-dim);
		margin-bottom: 4px;
	}

	.slider-head .val {
		color: var(--ink);
	}

	.range-mode {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		margin: 6px 0 7px;
		font-size: 10.5px;
	}

	.segment {
		display: inline-flex;
		border: 1px solid var(--line);
		border-radius: 2px;
		overflow: hidden;
		background: rgba(252, 251, 247, 0.55);
	}

	.segment button {
		padding: 2px 8px;
		border: 0;
		background: transparent;
		color: var(--ink-dim);
		font: inherit;
		cursor: pointer;
	}

	.segment button + button {
		border-left: 1px solid var(--line);
	}

	.segment button.active {
		background: var(--accent-soft);
		color: var(--accent);
	}

	input[type='range'] {
		-webkit-appearance: none;
		appearance: none;
		width: 100%;
		height: 4px;
		background: var(--line);
		border-radius: 2px;
		outline-offset: 4px;
		margin: 6px 0;
	}

	input[type='range']::-webkit-slider-thumb {
		-webkit-appearance: none;
		appearance: none;
		width: 14px;
		height: 14px;
		border-radius: 50%;
		background: var(--accent);
		border: 2px solid #fcfbf7;
		box-shadow: 0 1px 4px rgba(32, 36, 43, 0.3);
		cursor: ew-resize;
	}

	input[type='range']::-moz-range-thumb {
		width: 14px;
		height: 14px;
		border-radius: 50%;
		background: var(--accent);
		border: 2px solid #fcfbf7;
		box-shadow: 0 1px 4px rgba(32, 36, 43, 0.3);
		cursor: ew-resize;
	}

	.pred {
		margin: 2px 0 0;
		font-size: 11px;
		min-height: 16px;
	}

	.score {
		margin: 8px 0 0;
		font-size: 11px;
		color: var(--ink);
		min-height: 16px;
	}

	.demand-feedback {
		min-height: 78px;
	}

	.reset-row {
		min-height: 28px;
	}

	.movers-block {
		min-height: 114px;
	}

	.solvecard {
		position: absolute;
		top: 64px;
		right: 20px;
		z-index: 10;
		width: 240px;
		padding: 12px 14px 10px;
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: 3px;
		backdrop-filter: blur(6px);
		box-shadow: 0 4px 24px rgba(32, 36, 43, 0.08);
		animation: rise 0.3s ease-out both;
	}

	.solvecard-head {
		display: flex;
		justify-content: space-between;
		font-size: 11px;
		margin-bottom: 6px;
	}

	.solve-meta {
		display: flex;
		gap: 12px;
		font-size: 10px;
		margin-top: 4px;
	}

	.fallback-reason {
		margin: 6px 0 0;
		font-size: 10px;
		line-height: 1.35;
		overflow-wrap: anywhere;
	}

	.reset {
		margin-top: 10px;
		font-size: 10.5px;
		padding: 3px 10px;
		background: none;
		border: 1px solid var(--line);
		border-radius: 2px;
		color: var(--ink-dim);
		cursor: pointer;
	}

	.reset:hover {
		border-color: var(--red);
		color: var(--red);
	}

	table {
		width: 100%;
		margin-top: 12px;
		border-collapse: collapse;
		font-size: 12px;
	}

	td {
		padding: 3px 0;
		border-top: 1px solid var(--line);
	}

	td:last-child {
		text-align: right;
	}

	.pos {
		color: var(--pos);
	}

	.neg {
		color: var(--neg);
	}

	.sizes {
		display: flex;
		align-items: center;
		gap: 12px;
		font-size: 10px;
		color: var(--ink-dim);
	}

	.size {
		display: inline-flex;
		align-items: center;
		gap: 5px;
	}

	.size i {
		display: inline-block;
		border-radius: 50%;
		background: rgba(212, 116, 34, 0.55);
		border: 1px solid rgba(46, 42, 34, 0.45);
	}

	.caption {
		margin-left: auto;
		font-size: 9.5px;
	}

	footer {
		position: absolute;
		bottom: 0;
		left: 0;
		right: 0;
		z-index: 10;
		display: flex;
		align-items: center;
		padding: 8px 20px;
		font-size: 10.5px;
		color: var(--ink-dim);
		background: linear-gradient(rgba(236, 233, 226, 0), rgba(236, 233, 226, 0.9));
		animation: rise 0.5s 0.24s ease-out both;
		pointer-events: none;
	}

	footer a {
		pointer-events: auto;
		color: var(--ink-dim);
	}

	footer a:hover {
		color: var(--accent);
	}

	.sep {
		width: 4px;
		height: 4px;
		margin: 0 10px;
		background: var(--accent-bright);
		opacity: 0.55;
		transform: rotate(45deg);
	}

	.drophint {
		color: var(--ink-faint);
	}

	.drophint .arrow {
		color: var(--accent);
	}

	.dropzone {
		position: fixed;
		inset: 0;
		z-index: 20;
		pointer-events: none;
		background: rgba(236, 233, 226, 0.75);
	}

	.dropframe {
		position: absolute;
		inset: 14px;
		border: 1.5px dashed var(--accent);
		border-radius: 3px;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 6px;
	}

	.dropframe p {
		margin: 0;
		font-size: 13px;
	}

	.dropframe .hint {
		font-size: 11px;
		color: var(--ink-dim);
	}

	.placement-cue {
		position: absolute;
		left: 50%;
		bottom: 52px;
		z-index: 14;
		transform: translateX(-50%);
		padding: 8px 12px;
		background: var(--panel);
		border: 1px solid var(--accent);
		border-radius: 3px;
		color: var(--accent);
		font-size: 11px;
		box-shadow: 0 4px 18px rgba(32, 36, 43, 0.1);
		pointer-events: none;
	}

	.blink {
		animation: blink 1.2s steps(2) infinite;
	}

	@keyframes drop {
		from {
			opacity: 0;
			transform: translateY(-8px);
		}
	}

	@keyframes rise {
		from {
			opacity: 0;
			transform: translateY(8px);
		}
	}

	@keyframes blink {
		50% {
			opacity: 0.35;
		}
	}

	@keyframes bob {
		to {
			transform: translateY(2px);
		}
	}

	@media (max-width: 760px) {
		header {
			align-items: flex-start;
			flex-wrap: wrap;
			gap: 8px;
			padding: 8px 10px 12px;
			background: linear-gradient(rgba(236, 233, 226, 0.97), rgba(236, 233, 226, 0.72));
		}

		.brand {
			gap: 8px;
		}

		h1 {
			font-size: 20px;
		}

		.kicker {
			margin-left: auto;
			font-size: 9.5px;
			letter-spacing: 0;
			line-height: 2;
		}

		.cases {
			order: 3;
			width: 100%;
			overflow-x: auto;
			padding-bottom: 2px;
			scrollbar-width: none;
			scroll-padding: 10px;
			scroll-snap-type: x proximity;
			-webkit-overflow-scrolling: touch;
		}

		.cases::-webkit-scrollbar {
			display: none;
		}

		.cases > button,
		.case-chip {
			flex: 0 0 auto;
			max-width: 150px;
			min-height: 40px;
			scroll-snap-align: start;
		}

		.cases > button {
			padding: 7px 10px 6px;
		}

		.local-activate {
			padding: 7px 7px 6px 10px;
		}

		.local-remove {
			min-width: 34px;
		}

		.cname,
		.cregion {
			max-width: 100%;
			white-space: nowrap;
			overflow: hidden;
			text-overflow: ellipsis;
		}

		.panel {
			top: auto;
			left: 10px;
			right: 10px;
			bottom: 40px;
			width: auto;
			max-height: 44dvh;
			padding: 14px 16px;
		}

		.solvecard {
			top: 124px;
			left: auto;
			right: 10px;
			width: min(230px, calc(100% - 20px));
		}

		.mode {
			flex-wrap: wrap;
			gap: 7px;
		}

		.mode button {
			margin-left: 0;
		}

		.sizes {
			flex-wrap: wrap;
			gap: 8px 12px;
		}

		.caption {
			margin-left: 0;
			flex-basis: 100%;
		}

		footer {
			padding: 7px 10px;
			overflow-x: auto;
			font-size: 9.5px;
			white-space: nowrap;
			pointer-events: auto;
			scrollbar-width: none;
		}

		footer::-webkit-scrollbar {
			display: none;
		}

		.sep {
			margin: 0 8px;
		}

		.filedrop-ui,
		.filedrop-note {
			display: none;
		}
	}

	@media (hover: none), (pointer: coarse) {
		.filedrop-ui,
		.filedrop-note {
			display: none;
		}
	}

	@media (max-width: 420px) {
		.kicker {
			display: none;
		}

		.cases > button,
		.case-chip {
			max-width: 132px;
		}

		.panel {
			bottom: 34px;
			max-height: 46dvh;
		}

		.placement-cue {
			bottom: 38px;
			width: calc(100% - 28px);
			text-align: center;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		header,
		.panel,
		footer,
		.arrow {
			animation: none;
		}
	}
</style>
