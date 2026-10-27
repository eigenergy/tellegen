//! Assemble the PowerIO network from the drafts: buses with locations,
//! branches with routes, loads, generators with cost curves, the planned
//! shunt banks, optional HVDC links, geographic metadata, stable row
//! identities and the element annotations the report carries.

use std::collections::BTreeMap;

use powerio::geo::{CoordinateSpace, CoordsKind, GeoMeta, Location};
use powerio::{
    BalancedNetwork, Branch, Bus, BusId, BusType, GenCost, Generator, Hvdc, Load, Shunt,
};
use powerio_tx::GeneratorEnergySource;

use crate::electrical::branches::DraftBranch;
use crate::electrical::generators::DraftGenerator;
use crate::electrical::params::{BUS_VMAX, BUS_VMIN, GENERATOR_VG, RATE_B_FACTOR, RATE_C_FACTOR};
use crate::electrical::reactive::DraftShunt;
use crate::error::SynthError;
use crate::geom::wrap_lon;
use crate::report::{
    BranchAnnotation, BusAnnotation, CostCoefficients, ElementAnnotations, GeneratorAnnotation,
};
use crate::request::Fuel;
use crate::site::DraftBus;

/// An HVDC link draft (OSM path).
#[derive(Clone, Debug, PartialEq)]
pub struct DraftHvdc {
    pub from: usize,
    pub to: usize,
    pub rating_mw: f64,
    pub setpoint_mw: f64,
    /// Circuits of the mapped DC ways joined into this link.
    pub circuits: u32,
    pub osm_ids: Vec<i64>,
}

/// Key under which element extras are stored in the module.
pub const EXTRAS_KEY: &str = "tellegen_synth";

pub struct BuiltNetwork {
    pub net: BalancedNetwork,
    pub annotations: ElementAnnotations,
    /// Fuel per generator row.
    pub gen_fuels: Vec<Fuel>,
}

pub fn energy_source(fuel: Fuel) -> GeneratorEnergySource {
    match fuel {
        Fuel::Gas | Fuel::Coal => GeneratorEnergySource::Thermal,
        Fuel::Nuclear => GeneratorEnergySource::Nuclear,
        Fuel::Hydro => GeneratorEnergySource::Hydro,
        Fuel::Wind => GeneratorEnergySource::Wind,
        Fuel::Solar => GeneratorEnergySource::Solar,
        Fuel::Storage | Fuel::Other => GeneratorEnergySource::Other,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build_network(
    name: &str,
    buses: &[DraftBus],
    branches: &[DraftBranch],
    generators: &[DraftGenerator],
    shunts: &[DraftShunt],
    hvdc: &[DraftHvdc],
    coords_kind: CoordsKind,
    branch_osm_ids: Option<&[Vec<i64>]>,
    makeup_flags: Option<&[bool]>,
) -> Result<BuiltNetwork, SynthError> {
    let mut net = BalancedNetwork::new(name, crate::electrical::branches::BASE_MVA);
    let total_load: f64 = buses.iter().map(|b| b.load_mw).sum();
    let total_cap: f64 = generators.iter().map(|g| g.pmax_mw).sum();
    let dispatch_share = if total_cap > 0.0 {
        (total_load * 1.03 / total_cap).min(1.0)
    } else {
        0.0
    };

    let mut slack_buses = std::collections::HashSet::new();
    let mut gen_buses = std::collections::HashSet::new();
    for g in generators {
        gen_buses.insert(g.bus);
        if g.slack {
            slack_buses.insert(g.bus);
        }
    }

    let bus_rows = net.buses_mut();
    for (i, b) in buses.iter().enumerate() {
        let kind = if slack_buses.contains(&i) {
            BusType::Ref
        } else if gen_buses.contains(&i) {
            BusType::Pv
        } else {
            BusType::Pq
        };
        let mut bus = Bus::new(BusId(i + 1), kind, b.base_kv);
        bus.vmax = BUS_VMAX;
        bus.vmin = BUS_VMIN;
        bus.name = Some(b.name.clone());
        bus.location = Some(Location {
            x: wrap_lon(b.lon),
            y: b.lat,
            kind: Some(coords_kind),
        });
        bus.extras.insert(
            EXTRAS_KEY.to_owned(),
            serde_json::json!({
                "kind": b.kind.as_str(),
                "annotation": b.annotation,
                "confidence": round4(b.confidence),
                "place": b.place,
                "population_weight": round6(b.population_weight),
                "tier": b.tier,
                "island": b.island,
                "source": b.source,
            }),
        );
        bus_rows.push(bus);
    }

    let load_rows = net.loads_mut();
    for (i, b) in buses.iter().enumerate() {
        if b.load_mw > 1e-9 {
            let mut load = Load::new(BusId(i + 1), b.load_mw, b.load_mvar);
            load.extras.insert(
                EXTRAS_KEY.to_owned(),
                serde_json::json!({ "population_weight": round6(b.population_weight) }),
            );
            load_rows.push(load);
        }
    }

    let branch_rows = net.branches_mut();
    for (k, br) in branches.iter().enumerate() {
        let mut row = Branch::new(BusId(br.from + 1), BusId(br.to + 1), br.r_pu, br.x_pu);
        row.b = br.b_pu;
        row.rate_a = br.rate_a;
        row.rate_b = br.rate_a * RATE_B_FACTOR;
        row.rate_c = br.rate_a * RATE_C_FACTOR;
        row.tap = if br.is_transformer { 1.0 } else { 0.0 };
        row.route = Some(
            br.route
                .iter()
                .map(|&(lon, lat)| Location {
                    x: wrap_lon(lon),
                    y: lat,
                    kind: Some(coords_kind),
                })
                .collect(),
        );
        let osm_ids = branch_osm_ids
            .and_then(|ids| ids.get(k))
            .cloned()
            .unwrap_or_default();
        row.extras.insert(
            EXTRAS_KEY.to_owned(),
            serde_json::json!({
                "length_km": round3(br.length_km),
                "class_kv": br.class_kv,
                "nominal_kv": br.kv,
                "boundary_role": br.role.as_str(),
                "is_transformer": br.is_transformer,
                "transformer_mva": br.transformer_mva.map(round3),
                "winding_ratio": br.winding_ratio,
                "circuits": br.circuits,
                "circuit_index": br.circuit_index,
                "rating_basis": br.rating_basis,
                "osm_ids": osm_ids,
            }),
        );
        branch_rows.push(row);
    }

    let gen_rows = net.generators_mut();
    for g in generators {
        let mut row = Generator::new(BusId(g.bus + 1));
        row.energy_source = energy_source(g.fuel);
        row.pmax = g.pmax_mw;
        row.pmin = g.pmin_mw;
        row.qmax = g.qmax_mvar;
        row.qmin = g.qmin_mvar;
        row.vg = GENERATOR_VG;
        row.mbase = g.pmax_mw;
        row.pg = g.pmax_mw * dispatch_share;
        row.qg = 0.0;
        row.cost = Some(GenCost::new(2, 0.0, 0.0, vec![g.c2, g.c1, g.c0]));
        gen_rows.push(row);
    }

    let shunt_rows = net.shunts_mut();
    for sh in shunts {
        let mut row = Shunt::new(BusId(sh.bus + 1), 0.0, sh.mvar);
        row.extras.insert(
            EXTRAS_KEY.to_owned(),
            serde_json::json!({ "kind": sh.kind(), "basis": "reactive_balance" }),
        );
        shunt_rows.push(row);
    }

    let hvdc_rows = net.hvdc_mut();
    for link in hvdc {
        let mut row = Hvdc::new(BusId(link.from + 1), BusId(link.to + 1));
        row.pmax = link.rating_mw;
        row.pmin = -link.rating_mw;
        row.pf = link.setpoint_mw;
        row.pt = link.setpoint_mw;
        row.extras.insert(
            EXTRAS_KEY.to_owned(),
            serde_json::json!({ "osm_ids": link.osm_ids, "source": "osm", "circuits": link.circuits }),
        );
        hvdc_rows.push(row);
    }

    net.assign_missing_component_ids();
    *net.geo_mut() = Some(GeoMeta {
        space: CoordinateSpace::Geographic { crs: None },
        kind: Some(coords_kind),
    });
    net.validate()
        .map_err(|e| SynthError::Build(e.to_string()))?;
    tellegen::validate_canonical_identity(&net).map_err(SynthError::Build)?;

    let annotations = annotate(
        &net,
        buses,
        branches,
        generators,
        hvdc,
        branch_osm_ids,
        makeup_flags,
    );
    Ok(BuiltNetwork {
        net,
        annotations,
        gen_fuels: generators.iter().map(|g| g.fuel).collect(),
    })
}

fn annotate(
    net: &BalancedNetwork,
    buses: &[DraftBus],
    branches: &[DraftBranch],
    generators: &[DraftGenerator],
    hvdc: &[DraftHvdc],
    branch_osm_ids: Option<&[Vec<i64>]>,
    makeup_flags: Option<&[bool]>,
) -> ElementAnnotations {
    let uid = |u: &Option<String>| u.clone().unwrap_or_default();
    let bus_rows = net.buses();
    let branch_rows = net.branches();
    let gen_rows = net.generators();
    let hvdc_rows = net.hvdc();
    let mut out = ElementAnnotations {
        buses: buses
            .iter()
            .enumerate()
            .map(|(i, b)| BusAnnotation {
                id: i + 1,
                uid: uid(&bus_rows[i].uid),
                name: b.name.clone(),
                kind: b.kind,
                base_kv: b.base_kv,
                tier: b.tier,
                confidence: round4(b.confidence),
                source: b.source,
                place: b.place.clone(),
                population_weight: round6(b.population_weight),
                island: b.island,
            })
            .collect(),
        branches: branches
            .iter()
            .enumerate()
            .map(|(k, br)| BranchAnnotation {
                id: k + 1,
                uid: uid(&branch_rows[k].uid),
                class_kv: br.class_kv,
                length_km: round3(br.length_km),
                boundary_role: if br.is_transformer && br.length_km <= 0.0 {
                    "transformer".to_owned()
                } else {
                    br.role.as_str().to_owned()
                },
                is_transformer: br.is_transformer,
                is_hvdc: false,
                source: match br.role {
                    crate::topology::CorridorRole::Interconnect => {
                        crate::report::ElementSource::Interconnect
                    }
                    _ => buses[br.from].source,
                },
                circuits: br.circuits,
                circuit_index: br.circuit_index,
                rating_basis: br.rating_basis.to_owned(),
                osm_ids: branch_osm_ids
                    .and_then(|ids| ids.get(k))
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect(),
        generators: generators
            .iter()
            .enumerate()
            .map(|(k, g)| GeneratorAnnotation {
                uid: uid(&gen_rows[k].uid),
                bus: g.bus + 1,
                fuel: g.fuel,
                pmax_mw: round3(g.pmax_mw),
                cost: CostCoefficients {
                    c2: g.c2,
                    c1: g.c1,
                    c0: g.c0,
                },
                makeup: makeup_flags
                    .and_then(|f| f.get(k))
                    .copied()
                    .unwrap_or(g.makeup),
                slack: g.slack,
                condenser: g.condenser,
            })
            .collect(),
    };
    for (k, link) in hvdc.iter().enumerate() {
        out.branches.push(BranchAnnotation {
            id: branches.len() + k + 1,
            uid: uid(&hvdc_rows[k].uid),
            class_kv: 0.0,
            length_km: 0.0,
            boundary_role: "hvdc".to_owned(),
            is_transformer: false,
            is_hvdc: true,
            source: crate::report::ElementSource::Osm,
            circuits: link.circuits,
            circuit_index: 1,
            rating_basis: "osm".to_owned(),
            osm_ids: link.osm_ids.clone(),
        });
    }
    out
}

fn round3(x: f64) -> f64 {
    (x * 1e3).round() / 1e3
}

fn round4(x: f64) -> f64 {
    (x * 1e4).round() / 1e4
}

fn round6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

/// Generator fuel per row, read back from a module's report-less extras is
/// not possible (generators carry none), so callers keep `gen_fuels`.
pub fn dispatch_by_fuel(fuels: &[Fuel], dispatch: &[(usize, f64)]) -> BTreeMap<Fuel, f64> {
    let mut out = BTreeMap::new();
    for &(row, pg) in dispatch {
        if let Some(f) = fuels.get(row) {
            *out.entry(*f).or_insert(0.0) += pg;
        }
    }
    out
}
