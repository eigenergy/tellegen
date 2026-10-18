/** Proposed Tellegen contracts. Design material, not a shipped package API. */

// These four types mirror the proposed PowerIO wire contract. In production,
// generate them from the Rust schema rather than maintaining a second definition.
export interface ModelRef { network: string; revision: string }
export interface EquipmentRef { family: string; key: string }
export type SourceRef =
  | { kind: "bus"; bus: string }
  | { kind: "terminal"; bus: string; terminal: string }
  | { kind: "equipment"; equipment: EquipmentRef }
  | { kind: "part"; equipment: EquipmentRef; part_kind: string; part: string };
export interface PinRef { equipment: EquipmentRef; pin: string }

export type DiagramId = string;
export type PointId = string;
export type DiagramMode = "single_line" | "multi_wire";
export type Glyph =
  | "bus_bundle" | "terminal_rails" | "source" | "transformer"
  | "coil" | "load" | "ground" | "switch" | "opaque" | "breakout";

export interface DiagramSymbol {
  id: DiagramId;
  glyph: Glyph;
  label: string;
  sources: SourceRef[];
  group?: DiagramId;
  // Labels such as delta/wye supplement a glyph; they never define connections.
  badges: string[];
}

export interface DiagramPort {
  id: DiagramId;
  symbol: DiagramId;
  grouping: "individual" | "bundle" | "unresolved";
  // Membership means visual grouping, never an electrical union.
  points: PointId[];
  pins: PinRef[];
  side_hint?: "north" | "south" | "east" | "west";
  order_hint?: number;
}

export type WireMeaning =
  | { kind: "attachment" }
  | { kind: "conductor"; equipment: EquipmentRef; paths: string[] }
  | { kind: "switch_pole"; equipment: EquipmentRef; poles: string[] }
  | { kind: "grounding"; grounding: string };

export interface DiagramConnection {
  id: DiagramId;
  from: DiagramId;
  to: DiagramId;
  meaning: WireMeaning;
  sources: SourceRef[];
}

export interface DiagramGroup {
  id: DiagramId;
  parent?: DiagramId;
  label: string;
  sources: SourceRef[];
}

// This is a nonconducting annotation, never a DiagramConnection.
export interface MagneticDecoration {
  id: DiagramId;
  kind: "magnetic_coupling";
  symbols: DiagramId[];
  sources: SourceRef[];
}

export interface DetailNotice {
  sources: SourceRef[];
  reason: "collapsed" | "out_of_scope" | "unavailable";
  message: string;
}

export interface DiagramProjection {
  schema_version: 1;
  model_ref: ModelRef;
  mode: DiagramMode;
  symbols: DiagramSymbol[];
  ports: DiagramPort[];
  connections: DiagramConnection[];
  groups: DiagramGroup[];
  decorations: MagneticDecoration[];
  notices: DetailNotice[];
}

export interface ProjectionRequest {
  mode: DiagramMode;
  focus: SourceRef[];
  // Overrides support mixed-detail views without changing source connectivity.
  detail: { source: SourceRef; level: "compact" | "expanded" }[];
}

export interface XY { x: number; y: number }
export interface Box extends XY { width: number; height: number }
export interface DiagramLayout {
  projection_key: string;
  // Hash of measured glyph/label dimensions and relevant layout options.
  geometry_key: string;
  symbols: Record<DiagramId, Box & { rotation: 0 | 90 | 180 | 270 }>;
  ports: Record<DiagramId, XY>;
  routes: Record<DiagramId, XY[]>;
  labels: Record<DiagramId, XY>;
}

export interface HitTarget {
  visual: DiagramId;
  sources: SourceRef[];
}

export type ScenePrimitive =
  | { kind: "path"; id: string; d: string; style: string; hit?: HitTarget }
  | { kind: "text"; id: string; at: XY; text: string; style: string; hit?: HitTarget };

export interface DiagramScene {
  projection_key: string;
  bounds: Box;
  primitives: ScenePrimitive[];
}

// Proposed production boundary; gallery JSON uses a narrower fixture adapter.
export interface DiagramResultSnapshot {
  schema_version: 1;
  model_ref: ModelRef; // electrical identity, independent of layout/geometry_key
  result_key: string; // immutable solver snapshot digest
  provenance: { solver: string; revision: string; input_hash: string; created_at: string };
  status: "converged" | "stale" | "unavailable" | "rendering_stress";
  bus_voltages: {
    source: SourceRef;
    pairs: { terminals: [string, string]; magnitude_v: number; min_v?: number; max_v?: number }[];
  }[];
  branch_results: {
    source: SourceRef;
    // Positive at either end means INTO equipment; never derive one end from the other.
    ends: [DiagramEndPower | null, DiagramEndPower | null];
    loading: null | {
      ratio: number;
      basis: "max_conductor_current" | "aggregate_end_apparent_power";
      limiting_terminal?: string;
      rating_source: SourceRef;
    };
  }[];
}
export interface DiagramEndPower {
  p_w: number;
  q_var: number;
  // Include neutral; aggregate and individual quantities must remain distinguishable.
  terminals: { terminal: string; p_w: number; q_var: number; current_a: number }[];
}
