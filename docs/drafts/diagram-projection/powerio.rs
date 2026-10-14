//! Proposed PowerIO connectivity contract. Design material, not a shipped API.
//! IDs below are opaque; validation and projection constructors are not included.
use serde::{Deserialize, Serialize};

pub type PointId = String;
pub type LocalId = String;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRef {
    pub network: String,
    pub revision: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentRef {
    pub family: String,
    pub key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceRef {
    Bus {
        bus: String,
    },
    Terminal {
        bus: String,
        terminal: String,
    },
    Equipment {
        equipment: EquipmentRef,
    },
    Part {
        equipment: EquipmentRef,
        part_kind: String,
        part: LocalId,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Provenance {
    Declared,
    FormatConvention { name: String },
    Derived { rule: String },
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PointOwner {
    Bus { bus: String },
    Equipment { equipment: EquipmentRef },
    Reference { scope: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TerminalRole {
    Phase { designation: Option<String> },
    Neutral,
    ProtectiveEarth,
    EarthReference,
    Aggregate,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointNature {
    ExplicitTerminal,
    ModelOnly,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnectionPoint {
    pub id: PointId,
    pub owner: PointOwner,
    pub label: String,
    pub role: TerminalRole,
    pub nature: PointNature,
    pub provenance: Provenance,
    pub sources: Vec<SourceRef>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BusGroup {
    pub id: String,
    pub label: String,
    pub points: Vec<PointId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PinBinding {
    Connected { point: PointId },
    Disconnected,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pin {
    pub id: LocalId,
    pub binding: PinBinding,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnectionEnd {
    pub id: LocalId,
    pub label: String,
    pub pins: Vec<LocalId>,
}

/// A pin of this equipment, or an equipment-local/reference connection point.
/// External bus terminals must be reached through pins.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Contact {
    Pin { pin: LocalId },
    Point { point: PointId },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConductorNature {
    ExplicitConductor,
    ModelOnly,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConductorPath {
    pub id: LocalId,
    pub from_pin: LocalId,
    pub to_pin: LocalId,
    pub nature: ConductorNature,
    pub provenance: Provenance,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoleState {
    Open,
    Closed,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwitchPole {
    pub id: LocalId,
    pub from_pin: LocalId,
    pub to_pin: LocalId,
    pub state: PoleState,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionConfiguration {
    Wye,
    Delta,
    SinglePhase,
    Other,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoilPolarity {
    APositive,
    BPositive,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Coil {
    pub id: LocalId,
    pub a: Contact,
    pub b: Contact,
    pub polarity: CoilPolarity,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Winding {
    pub id: LocalId,
    pub end: LocalId,
    pub configuration: ConnectionConfiguration,
    pub coils: Vec<Coil>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoilRef {
    pub winding: LocalId,
    pub coil: LocalId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MagneticDetail {
    /// Transformer coupling exists, but coil association is not supplied.
    Unspecified,
    /// Diagram associations only; numerical coupling remains in the source model.
    CoilGroups {
        groups: Vec<Vec<CoilRef>>,
        provenance: Provenance,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerminalElement {
    pub id: LocalId,
    pub a: Contact,
    pub b: Contact,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EquipmentModel {
    ConductorPaths {
        paths: Vec<ConductorPath>,
    },
    Switch {
        poles: Vec<SwitchPole>,
    },
    Transformer {
        windings: Vec<Winding>,
        magnetic: MagneticDetail,
    },
    TerminalDevice {
        configuration: ConnectionConfiguration,
        elements: Vec<TerminalElement>,
    },
    Aggregate {
        description: String,
    },
    Opaque {
        reason: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    InService,
    OutOfService,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Equipment {
    pub id: EquipmentRef,
    pub label: String,
    pub availability: Availability,
    pub pins: Vec<Pin>,
    pub ends: Vec<ConnectionEnd>,
    pub model: EquipmentModel,
    pub provenance: Provenance,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GroundImpedance {
    Ideal,
    Impedance { r_ohm: f64, x_ohm: f64 },
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroundConnection {
    pub id: String,
    pub point: PointId,
    pub reference: PointId,
    pub impedance: GroundImpedance,
    pub sources: Vec<SourceRef>,
    pub provenance: Provenance,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnectivityDiagnostic {
    pub code: String,
    pub message: String,
    pub sources: Vec<SourceRef>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnectivityView {
    pub schema_version: u32,
    pub model_ref: ModelRef,
    pub buses: Vec<BusGroup>,
    pub points: Vec<ConnectionPoint>,
    pub equipment: Vec<Equipment>,
    pub grounding: Vec<GroundConnection>,
    pub diagnostics: Vec<ConnectivityDiagnostic>,
}
