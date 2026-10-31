// Generated from Rust by synth_schema and generate-synth-types.mjs.
// Source SHA256: f1e7810de0893fc37afbfdd86a3cbcd94cb20fdeb70aed089991a14234c9b8f5

export type AcPfOutcome = "converged" | "not_converged" | "skipped";

export type AcPfReport = { "buses_outside_emergency_band": number; "buses_outside_planning_band": number; "islands": Array<IslandOutcome>; "losses_mw"?: number | null; "message"?: string | null; "newton_iterations"?: number | null; "residual"?: number | null; "solve_ms"?: number | null; "status": AcPfOutcome; "vm_max"?: number | null; "vm_min"?: number | null; };

export type BBox = { "east": number; "north": number; "south": number; "west": number; };

export type BranchAnnotation = { /** `internal`, `tie_line` (leaves the region), `interconnect`, `transformer`. */ "boundary_role": string; /** 1-based position of this branch among its corridor's circuits (`1..=circuits`); 1 for an HVDC link. */ "circuit_index": number; /** Parallel circuits in this branch's corridor, this branch included; every branch of a corridor carries the same count. It is the corridor plan's count for a synthetic corridor and for the transformers and connections OpenStreetMap mode adds, and the summed `circuits` tags (else `cables / 3`) of the joined ways for a mapped OpenStreetMap corridor. An HVDC link is one row for all of its circuits. */ "circuits": number; "class_kv": number; /** 1-based branch position. */ "id": number; "is_hvdc": boolean; "is_transformer": boolean; "length_km": number; "osm_ids"?: Array<number>; /** `thermal`, `loadability` (St. Clair), `transformer`, `calibrated`. */ "rating_basis": string; "source": ElementSource; "uid": string; };

export type BusAnnotation = { "base_kv": number; "confidence": number; "id": number; /** Index of the electrical island (0 = largest). */ "island": number; "kind": BusKind; "name": string; "place"?: string | null; "population_weight": number; "source": ElementSource; /** 0 = highest voltage level. */ "tier": number; "uid": string; };

export type BusKind = "load" | "substation" | "generator" | "converter";

export type CalibrationOptions = { /** Allow the loop to scale total load (off: load is data, not a knob). */ "allow_load_scale"?: boolean; "enabled"?: boolean; /** Each iteration is one declared parameter step with an exact DC OPF re-solve. */ "max_iterations"?: number; /** Override the reference targets. */ "targets"?: (CalibrationTargets) | (null); };

export type CalibrationParameters = { /** Branches moved up one rating notch, by uid. */ "branch_rating_notches": { [key: string]: number; }; /** Capacity multiplier per fuel, bounds `[0.8, 1.5]`. */ "gen_capacity_scale": { "coal"?: number; "gas"?: number; "hydro"?: number; "nuclear"?: number; "other"?: number; "solar"?: number; "storage"?: number; "wind"?: number; }; /** Load multiplier; 1.0 unless `allow_load_scale`. */ "load_scale": number; /** Rating multiplier per line class (key = class kV), bounds `[0.8, 1.5]`. */ "rating_scale": { [key: string]: number; }; };

export type CalibrationReport = { "final_objective": number; "initial_objective": number; "iterations": number; "parameters": CalibrationParameters; "status": CalibrationStatus; "steps": Array<CalibrationStep>; "targets": CalibrationTargets; "terms_final": { [key: string]: number; }; "terms_initial": { [key: string]: number; }; /** Objective weights, declared up front. */ "weights": { [key: string]: number; }; };

export type CalibrationStatus = ("converged") | ("improved") | ("unchanged") | ("infeasible") | ("disabled");

export type CalibrationStep = { "accepted": boolean; "after": number; "before": number; "dc_status": SolveOutcome; "iteration": number; "objective_after": number; "objective_before": number; /** The parameter changed (`rating_scale[230]`, `gen_capacity_scale[gas]`, `branch_rating_notch[branches:12]`, `load_scale`). */ "parameter": string; "reason": string; };

export type CalibrationTargets = { /** Mean nodal degree range. */ "avg_degree": [number, number]; /** Fraction of branches at or above the loading target in the DC OPF. */ "binding_fraction": [number, number]; /** Fraction of buses at the highest voltage level. */ "hv_share": [number, number]; /** 90th percentile branch loading floor (under-utilisation guard). */ "loading_p90_min": number; /** Branch loading the calibration keeps the worst branch under. */ "loading_target": number; /** Identifier of the reference set (`reference_stats.json` entry). */ "set_id": string; /** Where the numbers come from. */ "source": string; };

export type CostCoefficients = { "c0": number; "c1": number; "c2": number; };

export type CountryDemand = { /** Share of the country's area the region covers (each part of the region counted to the country of its nearest place); absent without an area. */ "area_share"?: number | null; /** Country code of the places table (Natural Earth `adm0_a3`). */ "code": string; /** Population of the country's listed places over its national population, at most 1: the share of its peak that follows its places. */ "listed_share"?: number | null; /** The country's national peak (annual average load × peak factor), MW; `peak_mw` equals it when the region holds every place and at least 95% of the area of the country. Absent for the world fallback. */ "national_peak_mw"?: number | null; /** Peak demand of those places, MW. */ "peak_mw": number; "places": number; /** Sum of `pop_max` over those places. */ "population": number; /** Of `peak_mw`, the part of the country's unlisted demand the region takes: in proportion to the area it covers, never beyond its share of the country's listed population, MW. */ "unlisted_peak_mw"?: number | null; /** Year of the national figure; absent when the country has no entry and its places drew the world average. */ "year"?: number | null; };

export type DcOpfReport = { /** Branch uids with loading ≥ 0.999. */ "binding_branches": Array<string>; "binding_fraction": number; /** Per branch loading keyed by branch uid (the module's identity). */ "branch_loading"?: { [key: string]: number; }; "dispatch_by_fuel_mw": { "coal"?: number; "gas"?: number; "hydro"?: number; "nuclear"?: number; "other"?: number; "solar"?: number; "storage"?: number; "wind"?: number; }; /** HVDC rows replaced by a fixed transfer on the validation copy. */ "hvdc_as_fixed_transfer": number; "islands": Array<IslandOutcome>; "iterations"?: number | null; "lmp"?: (LmpSummary) | (null); /** Ten bins of width 0.1 from 0; the last bin holds loadings ≥ 0.9. */ "loading_histogram": Array<number>; "loading_mean"?: number | null; "loading_p90"?: number | null; "max_loading"?: number | null; "message"?: string | null; "objective"?: number | null; "solve_ms"?: number | null; "status": SolveOutcome; /** 0 when optimal, absent when infeasible (nothing is invented). */ "unserved_mw"?: number | null; };

export type DemandBasis = ("national") | ("population") | ("total_mw") | ("default_per_bus");

export type DemandSpec = ({ "kind": "population"; "peak_factor"?: number; "per_capita_kw"?: number | null; }) | ({ "kind": "total_mw"; "mw": number; });

export type DemandTableProvenance = { /** SHA-256 of the bundled country area table the unlisted demand spreads over. */ "areas_sha256": string; /** SHA-256 of the admin-0 file the areas were derived from. */ "areas_source_sha256": string; "licence": string; /** Peak demand as a multiple of the annual average load. */ "peak_factor": number; "schema": string; /** SHA-256 of the bundled table. */ "sha256": string; "source": string; /** SHA-256 of the dataset the table was derived from. */ "source_sha256": string; /** The year most entries report. */ "year": number; };

export type Density = "low" | "medium" | "high";

export type ElementAnnotations = { "branches": Array<BranchAnnotation>; "buses": Array<BusAnnotation>; "generators": Array<GeneratorAnnotation>; };

export type ElementSource = ("synthetic" | "osm") | ("makeup") | ("interconnect");

export type ErrorPayload = { "branches"?: number | null; "buses"?: number | null; "code": string; "message": string; };

export type FallbackFrom = { "branches"?: number | null; "buses"?: number | null; "message"?: string | null; "reason": FallbackReason; "source": Source; };

export type FallbackReason = "osm_insufficient" | "overpass_failed";

export type FarIslandPolicy = ("cut") | ("keep");

export type Fidelity = "low" | "high";

export type Fuel = "gas" | "coal" | "nuclear" | "hydro" | "wind" | "solar" | "storage" | "other";

export type GenerationReport = { "ac_pf"?: (AcPfReport) | (null); "calibration"?: (CalibrationReport) | (null); "context"?: (RegionContext) | (null); "dc_opf"?: (DcOpfReport) | (null); "elements"?: (ElementAnnotations) | (null); "planning"?: (PlanningReport) | (null); "provenance": Provenance; "request": SynthRequest; "schema": string; "stages": Array<StageRecord>; "stats"?: (TopologyStats) | (null); "warnings": Array<Warning>; };

export type GeneratorAnnotation = { "bus": number; /** A synchronous condenser the plan placed for voltage support in a remote pocket: no active power, reactive range only. */ "condenser"?: boolean; "cost": CostCoefficients; "fuel": Fuel; "makeup": boolean; "pmax_mw": number; "slack": boolean; "uid": string; };

export type IslandOptions = { "beyond"?: FarIslandPolicy; /** Landmasses whose nearest buses are closer than this are joined by an interconnect corridor (submarine or overhead), like Sicily or the Danish islands. source: typical AC/HVDC interconnector lengths, ENTSO-E grid map. */ "max_interconnect_km"?: number; };

export type IslandOutcome = { "branches": number; "buses": number; "generators": number; "index": number; "load_mw": number; "message"?: string | null; "status": string; };

export type Limits = { /** Spherical area cap of the region, km²; positive and finite. */ "max_area_km2"?: number; /** Upper bound of `branch_factor`; finite and at least 1. */ "max_branch_factor"?: number; /** Bus cap. A synthetic grid's count is held at it (`bus_count_capped`); an OpenStreetMap grid drops its lowest voltage levels until it fits (`osm_bus_count_capped`) and is refused when its highest level alone exceeds it. */ "max_bus_count"?: number; /** Maximum raw Overpass response size, in bytes. */ "max_osm_document_bytes"?: number; /** Maximum Overpass elements accepted before topology construction. */ "max_osm_elements"?: number; };

export type LmpSummary = { "max": number; "mean": number; "min": number; "spread": number; };

export type OsmIslands = ("connect") | ("cut") | ("keep");

export type OsmOptions = { /** Infrastructure this far outside the region polygon is still kept. */ "clip_buffer_km"?: number; /** Substation and converter elements within this radius collapse into one site. */ "cluster_radius_m"?: number; /** A DC line end within this distance of a converter snaps to it. */ "hvdc_snap_km"?: number; "islands"?: OsmIslands; /** Fewer branches than this is `OsmInsufficient`. */ "min_branches"?: number; /** Fewer buses than this is `OsmInsufficient`. */ "min_buses"?: number; /** A plant within this distance of a site connects there. */ "plant_snap_km"?: number; /** A line end within this distance of a site snaps to it. */ "snap_km"?: number; /** Lines and substations below this nominal voltage are distribution and are dropped. */ "voltage_floor_kv"?: number; };

export type OsmProvenance = { /** The Overpass answer's `osm3s.copyright` notice: the licence (ODbL) the data was served under. */ "copyright"?: string | null; "counts": { [key: string]: number; }; "document_sha256": string; "elements": number; "generator": string; "timestamp_osm_base"?: string | null; };

export type PlacesProvenance = { "count": number; /** SHA-256 of the table as loaded. */ "sha256": string; /** The dataset the table was derived from (the bundled table names Natural Earth's release); absent for a bare array of rows. */ "source"?: string | null; /** SHA-256 of that dataset. */ "source_sha256"?: string | null; };

export type PlanningReport = { /** The highest level the plan added above the modelled ones (one of `RegionContext::promotion_levels_kv`) because corridors at the highest modelled level needed more than two circuits; absent when the modelled levels sufficed or the region has no higher level in service. */ "added_level_kv"?: number | null; /** Widest island angle spread of the planning case (DC), degrees. */ "angle_spread_deg": number; /** Branches the requested branch factor asks for. */ "branch_target": number; /** Branches in the network. */ "branches": number; "capacitor_banks": number; /** Total capacitor rating, MVAr at 1 pu. */ "capacitor_mvar": number; /** Circuits added beyond one per corridor because the planning flow needed them. */ "capacity_circuits": number; /** Corridors at the cap whose planning flow still needs more circuits. */ "capped_corridors": number; /** Parallel circuits one corridor may hold. */ "circuit_cap": number; /** False when the plan stopped at its round or circuit cap. */ "converged": boolean; /** The planning case: `economic_dispatch_at_peak`, the least-cost dispatch of the generators serving peak load on a lossless network with no limits. */ "dispatch": string; /** Circuits added, heaviest planned loading first, to reach the target. */ "filler_circuits": number; /** Largest angle difference across one branch in the AC solution of the planning case at the planned voltage profile, degrees. */ "max_angle_difference_deg": number; /** Most parallel circuits on one corridor. */ "max_parallel_circuits": number; /** Highest circuit loading in the planning case. */ "max_planning_loading": number; /** Corridors still above their rating in the planning case. */ "overloaded_corridors": number; /** Buses raised to the next voltage level because an incident corridor would have needed more than two circuits at the lower one. */ "promoted_buses": number; "reactor_banks": number; /** Total shunt reactor rating, MVAr at 1 pu. */ "reactor_mvar": number; /** Capacity and stiffening rounds the corridor plan ran (each one DC power flow). */ "rounds": number; /** Circuits added to hold each island's angle spread within the St. Clair stability margin (44.4°). */ "stiffening_circuits": number; /** Transformer corridors whose bank the plan enlarged (up to the rating of the line it terminates) before adding circuits. */ "upsized_transformers": number; /** Share of its rating no circuit may exceed in the planning case. */ "utilisation_target": number; };

export type Preview = { "branches": Array<PreviewBranch>; "buses": Array<PreviewBus>; "stage": Stage; };

export type PreviewBranch = { "base_kv": number; "from": number; /** Longitude/latitude polyline. */ "route": Array<[number, number]>; "to": number; };

export type PreviewBus = { "base_kv": number; "id": number; "kind": BusKind; "lat": number; "lon": number; };

export type Provenance = { "demand_table"?: (DemandTableProvenance) | (null); "fallback_from"?: (FallbackFrom) | (null); "generator": string; "osm"?: (OsmProvenance) | (null); "parameter_table_version": string; "places"?: (PlacesProvenance) | (null); "powerio_version": string; "request_sha256": string; "seed": number; /** Log-normal spread applied to per-branch reactance. */ "spread_sigma": number; "tellegen_synth_version": string; "tellegen_version": string; "voltage_table"?: (VoltageTableProvenance) | (null); };

export type Quantiles = { "max": number; "mean": number; "min": number; "p10": number; "p50": number; "p90": number; };

export type ReferenceComparison = { "metric": string; "reference_range": [number, number]; "value": number; "within": boolean; };

export type Region = { /** GeoJSON geometry object (`{"type":"Polygon","coordinates":[...]}`). */ "geometry": unknown; "label"?: string; };

export type RegionContext = { /** Spherical area of the region polygons, km². */ "area_km2": number; /** Branches per bus the topology aims at. */ "branch_factor": number; "center_lat": number; "center_lon": number; "crosses_antimeridian": boolean; "demand_basis": DemandBasis; /** The countries behind the demand, largest peak first (national basis only). */ "demand_by_country"?: Array<CountryDemand>; /** Continuous longitude frame of the region. */ "frame": BBox; "fuel_mix": { "coal"?: number; "gas"?: number; "hydro"?: number; "nuclear"?: number; "other"?: number; "solar"?: number; "storage"?: number; "wind"?: number; }; "label": string; /** Polygons with usable area. */ "landmasses": number; "peak_demand_mw": number; "places_in_region": number; /** Places counted in the region although just outside its polygons: within 15 km of its edge, of a country whose place population it holds at least half of, and moved onto that edge (harbours and estuaries the polygons leave out). */ "places_snapped": number; /** Sum of `pop_max` over the populated places inside the region. */ "population_in_region": number; /** Higher AC levels the corridor plan may raise buses to, kV, highest first: the country's levels in service above its ladder, or 765 kV above stated levels. Empty when there is none, as in most of Europe. */ "promotion_levels_kv"?: Array<number>; "target_bus_count": number; /** Where the levels came from. */ "voltage_basis": VoltageBasis; /** The line class each level takes its per-km parameters from. */ "voltage_classes_kv": Array<number>; /** The country whose ladder the levels are (table code), or whose missing entry the fallback stands in for; absent for stated levels or a region without places. */ "voltage_country"?: string | null; /** Nominal levels, highest first; a level the corridor plan added above them (`PlanningReport::added_level_kv`) comes first once it ran. */ "voltage_levels_kv": Array<number>; };

export type SolveOutcome = "optimal" | "infeasible" | "skipped";

export type Source = ("synthetic") | ("osm");

export type Stage = "context" | "site_buses" | "topology" | "electrify" | "dc_opf" | "calibrate" | "ac_pf" | "finalize" | "done";

export type StageRecord = { /** Element counts and other integers the stage produced. */ "counts": { [key: string]: number; }; "elapsed_ms"?: number | null; "iteration"?: number | null; "stage": Stage; };

export type StepOutcome = { "done": boolean; /** Stage wall time when the host supplied a clock. */ "elapsed_ms"?: number | null; /** Calibration iteration (1-based) when `stage == Calibrate`. */ "iteration"?: number | null; /** The stage the next `step()` runs; `None` after `Done`. */ "next"?: (Stage) | (null); /** True when `preview()` would return something new. */ "preview_changed": boolean; /** Progress in `[0, 1]`. */ "progress": number; /** The stage that just ran. */ "stage": Stage; /** Warnings raised by this stage. */ "warnings": Array<Warning>; };

export type SynthRequest = { /** Branches per bus, `1.0..=limits.max_branch_factor`; otherwise by density. */ "branch_factor"?: number | null; "calibration"?: CalibrationOptions; "demand"?: DemandSpec; "density"?: Density; "fallback_from"?: (FallbackFrom) | (null); "fidelity"?: Fidelity; /** Capacity shares by fuel; normalised. Empty means the default mix. */ "fuel_mix"?: { "coal"?: number; "gas"?: number; "hydro"?: number; "nuclear"?: number; "other"?: number; "solar"?: number; "storage"?: number; "wind"?: number; }; /** Number of generating units; otherwise 16% of the buses (at least 3). */ "generator_count"?: number | null; "islands"?: IslandOptions; "limits"?: Limits; "osm"?: (OsmOptions) | (null); /** Load power factor; `q = p · tan(acos pf)`. */ "power_factor"?: number; "region": Region; /** Installed capacity ≥ (1 + margin) × peak load. */ "reserve_margin"?: number; "seed"?: number; "source"?: Source; /** Exact bus count; otherwise `density` × area decides. */ "target_bus_count"?: number | null; "validation"?: ValidationOptions; /** Nominal voltage levels, kV, highest first after resolution. Each level keeps its own kV for the per-unit base and takes the per-km parameters of the nearest line class. Empty (the default) takes the national ladder of the region's dominant country by demand ([`crate::voltage::VoltageLadders`]); a stated list is used as given, and the corridor plan may then add 765 kV above it. */ "voltage_levels_kv"?: Array<number>; };

export type TopologyStats = { /** Second smallest Laplacian eigenvalue, by sparse power iteration; `None` above the size cap or for an empty graph. */ "algebraic_connectivity"?: number | null; "algebraic_connectivity_converged"?: boolean | null; "avg_degree": number; "branch_bus_ratio": number; "clustering_coefficient": number; "connected_components": number; /** `degree_histogram[d]` = number of buses with degree `d`. */ "degree_histogram": Array<number>; /** Fraction of distinct corridors that are Delaunay edges of the bus points. */ "delaunay_edge_fraction": number; "gen_load_margin": number; /** Fraction of buses at the highest voltage level. */ "hv_share": number; "line_length_km": Quantiles; "max_degree": number; "max_line_loading"?: number | null; /** Total corridor length over the Euclidean minimum spanning tree length. */ "mst_length_ratio": number; "n_branches": number; "n_buses": number; "n_generators": number; "n_hvdc": number; "n_loads": number; "n_transformers": number; "parallel_branches": number; "parallel_corridors": number; "reference"?: Array<ReferenceComparison>; "total_generation_capacity_mw": number; "total_line_length_km": number; "total_load_mw": number; /** Buses per nominal voltage (key = kV). */ "voltage_counts": { [key: string]: number; }; };

export type ValidationOptions = { /** Emergency voltage band, per unit. */ "emergency_band"?: [number, number]; "run_ac_pf"?: boolean; "run_dc_opf"?: boolean; /** Planning voltage band, per unit. */ "voltage_band"?: [number, number]; };

export type VoltageBasis = ("request") | ("country") | ("fallback");

export type VoltageTableProvenance = { "schema": string; /** SHA-256 of the bundled table. */ "sha256": string; "version": string; };

export type Warning = { "code": string; "message": string; };
