//! Deterministic synthetic multiconductor feeders for the retained-session
//! benchmarks (eigenergy/tellegen#132).
//!
//! Every preset is generated from a fixed seed with an inline SplitMix64
//! generator, so the same preset always yields the same network, element
//! order, names, and load values. The shapes reproduce the networks the issue
//! measured:
//!
//! - [`Preset::Feeder10k`]: an 11 kV four-wire primary with 650 Dyn11
//!   11 kV/0.4 kV transformers and their low-voltage clusters (9,950 buses,
//!   32,412 terminals, 9,299 editable load branches).
//! - [`Preset::Feeder106k`]: a 12.47 kV four-wire primary with single-phase
//!   two-wire taps and tie lines (27,239 buses, 106,038 terminals).
//! - [`Preset::X300k`] and [`Preset::X650k`]: the `Feeder106k` style scaled to
//!   316,904 and 676,530 terminals, next to the SMART-DS Greensboro (316,905)
//!   and San Francisco P5U (676,529) conductor-node counts. These are
//!   exploratory scaling points only.
//! - [`Preset::Tiny`]: 60 buses with every load connection and voltage model,
//!   cheap enough to solve in debug-mode unit tests.
//!
//! Primary feeders grow radially: a new bus continues the previous bus's
//! chain with probability `chain_probability`, otherwise it branches from a
//! uniformly chosen earlier bus of the same feeder. Neutrals are named `n`,
//! which `DistBus::phase_indices` treats as non-phase. The source neutral,
//! every transformer secondary neutral, every fourth low-voltage four-wire
//! bus, and every fifth primary bus are solidly grounded (a multi-grounded
//! neutral). The networks contain no switches, which the fixed-point solver
//! rejects when closed.

use std::collections::BTreeSet;
use std::f64::consts::PI;

use powerio_dist::{
    Configuration, DistBus, DistLine, DistLineCode, DistLoad, DistLoadVoltageModel,
    DistTransformer, DistWinding, DistWindingConn, MulticonductorNetwork, VoltageSource,
};

/// The SplitMix64 generator (Steele, Lea, and Flood 2014). Small, fast, and
/// fully specified, so the browser benchmark can reproduce the same stream.
#[derive(Clone, Debug)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform on `[0, 1)` with 53 random bits.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform on `[lo, hi)`.
    pub fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f64()
    }

    /// Uniform integer on `0..n` (multiply-high reduction). `n` must be positive.
    pub fn below(&mut self, n: usize) -> usize {
        assert!(n > 0, "SplitMix64::below requires a positive bound");
        ((u128::from(self.next_u64()) * n as u128) >> 64) as usize
    }

    pub fn chance(&mut self, probability: f64) -> bool {
        self.next_f64() < probability
    }

    /// Fisher-Yates shuffle.
    pub fn shuffle<T>(&mut self, values: &mut [T]) {
        for i in (1..values.len()).rev() {
            let j = self.below(i + 1);
            values.swap(i, j);
        }
    }
}

/// Network size presets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Preset {
    Tiny,
    Feeder10k,
    Feeder106k,
    X300k,
    X650k,
}

impl Preset {
    pub const ALL: [Preset; 5] = [
        Preset::Tiny,
        Preset::Feeder10k,
        Preset::Feeder106k,
        Preset::X300k,
        Preset::X650k,
    ];

    /// The command-line and file-name identifier.
    pub fn name(self) -> &'static str {
        match self {
            Preset::Tiny => "tiny",
            Preset::Feeder10k => "feeder-10k",
            Preset::Feeder106k => "feeder-106k",
            Preset::X300k => "x300k",
            Preset::X650k => "x650k",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|preset| preset.name().eq_ignore_ascii_case(text.trim()))
    }

    /// The frozen load multiplier. Calibrated with `mc-pf-session-bench
    /// --calibrate` (release-py build, default `McPfOptions`) for a minimum
    /// load-branch voltage near 0.93 pu. Cold-solve results at these values:
    ///
    /// | preset | load_scale | iterations | min V (pu) |
    /// | --- | ---: | ---: | ---: |
    /// | tiny | 2.5 | 10 | 0.9315 |
    /// | feeder-10k | 0.7 | 9 | 0.9303 |
    /// | feeder-106k | 2.4 | 11 | 0.9307 |
    /// | x300k | 4.0 | 11 | 0.9316 |
    /// | x650k | 2.7 | 11 | 0.9314 |
    ///
    /// The primary-only presets are deliberately heavily loaded: their
    /// meshed primaries are electrically stiff, and the multiplier is a
    /// calibration knob for the voltage profile, not a load forecast.
    pub fn load_scale(self) -> f64 {
        match self {
            Preset::Tiny => 2.5,
            Preset::Feeder10k => 0.7,
            Preset::Feeder106k => 2.4,
            Preset::X300k => 4.0,
            Preset::X650k => 2.7,
        }
    }

    /// The generator parameters for this preset.
    pub fn spec(self) -> FeederSpec {
        let secondary = |four_wire_buses, two_wire_buses| SecondarySpec {
            kv_ll: 0.4,
            transformer_kva: 250.0,
            xsc_pct: 4.0,
            r_pct: 0.5,
            four_wire_buses,
            two_wire_buses,
            segment_m: (20.0, 45.0),
            service_m: (10.0, 30.0),
            ground_every: 4,
        };
        let transformer_style = |name: &str, seed, feeders, primary_buses| FeederSpec {
            name: name.to_owned(),
            seed,
            base_frequency: 50.0,
            primary_kv_ll: 11.0,
            feeders,
            primary_buses,
            primary_segment_m: (150.0, 400.0),
            primary_taps: 0,
            primary_tap_m: (30.0, 120.0),
            primary_ties: 0,
            min_tie_distance: 6,
            primary_ground_every: 5,
            chain_probability: 0.75,
            secondary: None,
            three_phase_loads: 0,
            single_phase_loads: 0,
            single_phase_kw: (1.5, 6.0),
            three_phase_kw_per_phase: (4.0, 15.0),
            wye_share: 0.6,
            load_scale: self.load_scale(),
        };
        let primary_style =
            |name: &str, seed, feeders, primary_buses, taps, ties, three_phase| FeederSpec {
                name: name.to_owned(),
                seed,
                base_frequency: 60.0,
                primary_kv_ll: 12.47,
                feeders,
                primary_buses,
                primary_segment_m: (30.0, 120.0),
                primary_taps: taps,
                primary_tap_m: (30.0, 120.0),
                primary_ties: ties,
                min_tie_distance: 6,
                primary_ground_every: 5,
                chain_probability: 0.9,
                secondary: None,
                three_phase_loads: three_phase,
                single_phase_loads: taps,
                single_phase_kw: (5.0, 25.0),
                three_phase_kw_per_phase: (10.0, 50.0),
                wye_share: 0.6,
                load_scale: self.load_scale(),
            };
        match self {
            Preset::Tiny => FeederSpec {
                secondary: Some(secondary(32, 21)),
                three_phase_loads: 8,
                single_phase_loads: 31,
                ..transformer_style("tellegen-mc-tiny", 0x7E11_E6E0_0000_0001, 2, 6)
            },
            Preset::Feeder10k => FeederSpec {
                secondary: Some(secondary(5_605, 3_694)),
                three_phase_loads: 789,
                single_phase_loads: 6_932,
                ..transformer_style("tellegen-mc-feeder-10k", 0x7E11_E6E0_0000_0010, 8, 650)
            },
            Preset::Feeder106k => primary_style(
                "tellegen-mc-feeder-106k",
                0x7E11_E6E0_0000_0106,
                12,
                25_779,
                1_459,
                565,
                644,
            ),
            Preset::X300k => primary_style(
                "tellegen-mc-x300k",
                0x7E11_E6E0_0000_0300,
                36,
                77_045,
                4_360,
                1_690,
                1_925,
            ),
            Preset::X650k => primary_style(
                "tellegen-mc-x650k",
                0x7E11_E6E0_0000_0650,
                76,
                164_477,
                9_309,
                3_605,
                4_109,
            ),
        }
    }

    /// The exact element counts this preset must generate.
    pub fn target_shape(self) -> FeederShape {
        let (buses, terminals, lines, transformers, loads, load_branches) = match self {
            Preset::Tiny => (60, 198, 53, 6, 39, 55),
            Preset::Feeder10k => (9_950, 32_412, 9_299, 650, 7_721, 9_299),
            Preset::Feeder106k => (27_239, 106_038, 27_803, 0, 2_103, 3_391),
            Preset::X300k => (81_406, 316_904, 83_095, 0, 6_285, 10_135),
            Preset::X650k => (173_787, 676_530, 177_391, 0, 13_418, 21_636),
        };
        FeederShape {
            buses,
            terminals,
            lines,
            transformers,
            loads,
            load_branches,
        }
    }
}

impl std::fmt::Display for Preset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// Low-voltage secondary clusters, one per primary bus, each fed by a
/// Dyn11 transformer whose secondary neutral is solidly grounded.
#[derive(Clone, Debug, PartialEq)]
pub struct SecondarySpec {
    pub kv_ll: f64,
    pub transformer_kva: f64,
    /// Leakage reactance, percent on the transformer rating.
    pub xsc_pct: f64,
    /// Resistance per winding, percent on the transformer rating.
    pub r_pct: f64,
    /// Four-wire secondary buses, including one transformer root per cluster.
    pub four_wire_buses: usize,
    /// Two-wire `[phase, n]` service-drop buses.
    pub two_wire_buses: usize,
    pub segment_m: (f64, f64),
    pub service_m: (f64, f64),
    /// Ground the neutral of every k-th four-wire secondary bus.
    pub ground_every: usize,
}

/// The generator parameters. [`Preset::spec`] supplies the frozen presets.
#[derive(Clone, Debug, PartialEq)]
pub struct FeederSpec {
    pub name: String,
    pub seed: u64,
    pub base_frequency: f64,
    pub primary_kv_ll: f64,
    /// Radial primary feeders leaving the source bus.
    pub feeders: usize,
    /// Four-wire primary buses, excluding the source bus.
    pub primary_buses: usize,
    pub primary_segment_m: (f64, f64),
    /// Two-wire `[phase, n]` single-phase primary taps.
    pub primary_taps: usize,
    pub primary_tap_m: (f64, f64),
    /// Tie lines closing loops between four-wire buses of one feeder.
    pub primary_ties: usize,
    /// Minimum tree distance, in lines, spanned by a tie.
    pub min_tie_distance: usize,
    /// Ground the neutral of every k-th primary bus.
    pub primary_ground_every: usize,
    /// Probability that a new bus continues the previous bus's chain; the
    /// rest attach to a uniformly chosen earlier bus of the same feeder.
    pub chain_probability: f64,
    pub secondary: Option<SecondarySpec>,
    /// Three-phase loads (wye with explicit neutral, or delta).
    pub three_phase_loads: usize,
    /// Single-phase `[phase, n]` loads; every two-wire bus carries one and
    /// the remainder sit on four-wire load buses.
    pub single_phase_loads: usize,
    /// Base kW range before `load_scale`.
    pub single_phase_kw: (f64, f64),
    pub three_phase_kw_per_phase: (f64, f64),
    /// Share of three-phase loads connected wye (the rest delta).
    pub wye_share: f64,
    pub load_scale: f64,
}

/// Element counts of a generated (or loaded) network.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct FeederShape {
    pub buses: usize,
    pub terminals: usize,
    pub lines: usize,
    pub transformers: usize,
    pub loads: usize,
    /// Editable load branches (one per prescribed branch power).
    pub load_branches: usize,
}

impl FeederShape {
    pub fn of(network: &MulticonductorNetwork) -> Self {
        Self {
            buses: network.buses().len(),
            terminals: network.buses().iter().map(|bus| bus.terminals.len()).sum(),
            lines: network.lines().len(),
            transformers: network.transformers().len(),
            loads: network.loads().len(),
            load_branches: network.loads().iter().map(|load| load.p_nom.len()).sum(),
        }
    }
}

impl FeederSpec {
    /// The element counts [`generate_spec`] produces, without generating.
    pub fn shape(&self) -> FeederShape {
        // A radial primary line per primary bus and tap, plus the ties; each
        // secondary cluster adds a line per bus except its transformer root.
        let (secondary_four, secondary_two, transformers) =
            self.secondary.as_ref().map_or((0, 0, 0), |s| {
                (s.four_wire_buses, s.two_wire_buses, self.primary_buses)
            });
        let four_wire = 1 + self.primary_buses + secondary_four;
        let two_wire = self.primary_taps + secondary_two;
        let lines = self.primary_buses
            + self.primary_taps
            + self.primary_ties
            + secondary_four
            + secondary_two
            - transformers;
        FeederShape {
            buses: four_wire + two_wire,
            terminals: 4 * four_wire + 2 * two_wire,
            lines,
            transformers,
            loads: self.three_phase_loads + self.single_phase_loads,
            load_branches: 3 * self.three_phase_loads + self.single_phase_loads,
        }
    }
}

/// Generate one preset.
pub fn generate(preset: Preset) -> MulticonductorNetwork {
    generate_spec(&preset.spec())
}

const PHASES: [&str; 3] = ["a", "b", "c"];
const NEUTRAL: &str = "n";

fn terminals(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn four_wire() -> Vec<String> {
    terminals(&["a", "b", "c", NEUTRAL])
}

fn two_wire(phase: &str) -> Vec<String> {
    terminals(&[phase, NEUTRAL])
}

/// An `n`-conductor linecode in ohm per metre from per-kilometre phase,
/// neutral, and mutual impedances. The last conductor is the neutral.
fn linecode(
    name: &str,
    conductors: usize,
    phase: (f64, f64),
    neutral: (f64, f64),
    mutual: (f64, f64),
) -> DistLineCode {
    let per_m = 1e-3;
    let mut r = vec![vec![mutual.0 * per_m; conductors]; conductors];
    let mut x = vec![vec![mutual.1 * per_m; conductors]; conductors];
    for i in 0..conductors {
        let own = if i + 1 == conductors { neutral } else { phase };
        r[i][i] = own.0 * per_m;
        x[i][i] = own.1 * per_m;
    }
    DistLineCode::new(name, r, x)
}

/// Split `total` into `parts` near-equal sizes; the `total % parts` larger
/// shares go to a seeded random subset.
fn split_sizes(rng: &mut SplitMix64, total: usize, parts: usize) -> Vec<usize> {
    let base = total / parts;
    let mut order: Vec<usize> = (0..parts).collect();
    rng.shuffle(&mut order);
    let mut sizes = vec![base; parts];
    for &part in order.iter().take(total % parts) {
        sizes[part] += 1;
    }
    sizes
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModelKind {
    ConstantPower,
    ConstantCurrent,
    ConstantImpedance,
    Zip,
    Exponential,
}

/// Exact voltage-model quotas (70/10/10/5/5 percent, each voltage-dependent
/// model at least once when there are enough loads), in seeded order.
fn model_kinds(rng: &mut SplitMix64, loads: usize) -> Vec<ModelKind> {
    let share = |fraction: f64| {
        let count = (fraction * loads as f64).round() as usize;
        if loads >= 10 {
            count.max(1)
        } else {
            count
        }
    };
    let quotas = [
        (ModelKind::ConstantCurrent, share(0.10)),
        (ModelKind::ConstantImpedance, share(0.10)),
        (ModelKind::Zip, share(0.05)),
        (ModelKind::Exponential, share(0.05)),
    ];
    let mut kinds = Vec::with_capacity(loads);
    for (kind, count) in quotas {
        kinds.extend(std::iter::repeat_n(kind, count));
    }
    assert!(
        kinds.len() <= loads,
        "voltage-model quotas exceed the load count"
    );
    kinds.resize(loads, ModelKind::ConstantPower);
    rng.shuffle(&mut kinds);
    kinds
}

struct PrimaryTree {
    /// Parent primary index; `None` attaches to the source bus.
    parent: Vec<Option<usize>>,
    depth: Vec<usize>,
    /// Length of the line to the parent, metres.
    length: Vec<f64>,
    feeder: Vec<usize>,
    position: Vec<usize>,
    members: Vec<Vec<usize>>,
}

impl PrimaryTree {
    /// Tree distance between two primary buses as (lines, metres).
    fn distance(&self, mut a: usize, mut b: usize) -> (usize, f64) {
        let (mut hops, mut metres) = (0, 0.0);
        while self.depth[a] > self.depth[b] {
            metres += self.length[a];
            a = self.parent[a].expect("deeper bus has a primary parent");
            hops += 1;
        }
        while self.depth[b] > self.depth[a] {
            metres += self.length[b];
            b = self.parent[b].expect("deeper bus has a primary parent");
            hops += 1;
        }
        while a != b {
            metres += self.length[a] + self.length[b];
            hops += 2;
            match (self.parent[a], self.parent[b]) {
                (Some(pa), Some(pb)) => {
                    a = pa;
                    b = pb;
                }
                // Both reach the source bus at depth zero.
                _ => break,
            }
        }
        (hops, metres)
    }
}

struct Builder {
    buses: Vec<DistBus>,
    lines: Vec<DistLine>,
    transformers: Vec<DistTransformer>,
    loads: Vec<DistLoad>,
}

impl Builder {
    fn bus(&mut self, id: String, terminals: Vec<String>, grounded: bool) {
        let mut bus = DistBus::new(id, terminals);
        if grounded {
            bus.grounded.push(NEUTRAL.to_owned());
        }
        self.buses.push(bus);
    }

    fn line(&mut self, from: &str, to: &str, map: Vec<String>, code: &str, length: f64) {
        let name = format!("l{}", self.lines.len());
        self.lines.push(DistLine::new(
            name,
            from,
            to,
            map.clone(),
            map,
            code,
            length,
        ));
    }
}

/// Where a load connects.
enum LoadSite {
    /// A two-wire `[phase, n]` bus.
    TwoWire { bus: String, phase: &'static str },
    /// A four-wire bus.
    FourWire { bus: String },
}

/// Generate a network from explicit parameters.
pub fn generate_spec(spec: &FeederSpec) -> MulticonductorNetwork {
    assert!(spec.feeders > 0 && spec.primary_buses >= spec.feeders);
    assert!(spec.primary_ground_every > 0);
    let mut rng = SplitMix64::new(spec.seed);
    let primary_vll = spec.primary_kv_ll * 1e3;
    let mut builder = Builder {
        buses: Vec::with_capacity(spec.shape().buses),
        lines: Vec::with_capacity(spec.shape().lines),
        transformers: Vec::new(),
        loads: Vec::new(),
    };

    // Source bus: 1 four-wire bus, solidly grounded neutral.
    builder.bus("src".to_owned(), four_wire(), true);

    // Radial four-wire primary feeders.
    let sizes = split_sizes(&mut rng, spec.primary_buses, spec.feeders);
    let mut tree = PrimaryTree {
        parent: Vec::with_capacity(spec.primary_buses),
        depth: Vec::with_capacity(spec.primary_buses),
        length: Vec::with_capacity(spec.primary_buses),
        feeder: Vec::with_capacity(spec.primary_buses),
        position: Vec::with_capacity(spec.primary_buses),
        members: Vec::with_capacity(spec.feeders),
    };
    for (feeder, &size) in sizes.iter().enumerate() {
        let mut members = Vec::with_capacity(size);
        for k in 0..size {
            let index = tree.parent.len();
            let parent = if k == 0 {
                None
            } else if rng.chance(spec.chain_probability) {
                Some(members[k - 1])
            } else {
                Some(members[rng.below(k)])
            };
            let depth = parent.map_or(1, |parent: usize| tree.depth[parent] + 1);
            tree.depth.push(depth);
            tree.parent.push(parent);
            tree.feeder.push(feeder);
            tree.position.push(k);
            members.push(index);
            let grounded = (index + 1).is_multiple_of(spec.primary_ground_every);
            builder.bus(format!("p{index}"), four_wire(), grounded);
            let from = parent.map_or_else(|| "src".to_owned(), |parent| format!("p{parent}"));
            let length = rng.uniform(spec.primary_segment_m.0, spec.primary_segment_m.1);
            tree.length.push(length);
            builder.line(&from, &format!("p{index}"), four_wire(), "mv4", length);
        }
        tree.members.push(members);
    }

    let mut two_wire_sites = Vec::new();
    let mut four_wire_sites = Vec::new();

    // Single-phase two-wire primary taps.
    for tap in 0..spec.primary_taps {
        let parent = rng.below(spec.primary_buses);
        let phase = PHASES[tap % 3];
        let id = format!("t{tap}");
        builder.bus(id.clone(), two_wire(phase), false);
        let length = rng.uniform(spec.primary_tap_m.0, spec.primary_tap_m.1);
        builder.line(&format!("p{parent}"), &id, two_wire(phase), "mv2", length);
        two_wire_sites.push(LoadSite::TwoWire { bus: id, phase });
    }

    // Primary ties between nearby (in creation order) buses of one feeder
    // that are at least `min_tie_distance` lines apart in the tree. A tie
    // follows a parallel route: 50-100 % of the tree path it closes.
    let mut tied = BTreeSet::new();
    for _ in 0..spec.primary_ties {
        let mut attempts = 0usize;
        loop {
            attempts += 1;
            assert!(attempts < 1_000_000, "cannot place the requested ties");
            let u = rng.below(spec.primary_buses);
            let members = &tree.members[tree.feeder[u]];
            let offset = spec.min_tie_distance + rng.below(60);
            let Some(&v) = members.get(tree.position[u] + offset) else {
                continue;
            };
            let (hops, path_m) = tree.distance(u, v);
            if hops < spec.min_tie_distance || !tied.insert((u.min(v), u.max(v))) {
                continue;
            }
            let length = path_m * rng.uniform(0.5, 1.0);
            builder.line(
                &format!("p{u}"),
                &format!("p{v}"),
                four_wire(),
                "mv4",
                length,
            );
            break;
        }
    }

    // Low-voltage clusters, one per primary bus, behind a Dyn11 transformer.
    if let Some(secondary) = &spec.secondary {
        // Secondary loads use the secondary voltage base; primary taps would
        // need their own and are not part of the transformer-style presets.
        assert_eq!(
            spec.primary_taps, 0,
            "secondary presets carry no primary taps"
        );
        assert!(secondary.four_wire_buses >= spec.primary_buses);
        assert!(secondary.ground_every > 0);
        let four_sizes = split_sizes(&mut rng, secondary.four_wire_buses, spec.primary_buses);
        let two_sizes = split_sizes(&mut rng, secondary.two_wire_buses, spec.primary_buses);
        let mut lv_four_wire = 0usize;
        let mut drops = 0usize;
        for cluster in 0..spec.primary_buses {
            let root = format!("s{cluster}_0");
            let mut extras = std::collections::BTreeMap::new();
            // The BMOPF `delta_wye` subtype fixes the delta orientation
            // exactly as the oracle-validated `pf_dy_xfmr` fixture maps.
            extras.insert(
                "bmopf_subtype".to_owned(),
                serde_json::Value::String("delta_wye".to_owned()),
            );
            let rating = secondary.transformer_kva * 1e3;
            let mut high = DistWinding::new(
                format!("p{cluster}"),
                terminals(&PHASES),
                DistWindingConn::Delta,
                primary_vll,
                rating,
            );
            high.r_pct = secondary.r_pct;
            let mut low = DistWinding::new(
                root.clone(),
                four_wire(),
                DistWindingConn::Wye,
                secondary.kv_ll * 1e3,
                rating,
            );
            low.r_pct = secondary.r_pct;
            let mut transformer = DistTransformer::new(
                format!("x{cluster}"),
                vec![high, low],
                vec![secondary.xsc_pct],
                3,
            );
            transformer.extras = extras;
            builder.transformers.push(transformer);

            let mut members: Vec<String> = Vec::with_capacity(four_sizes[cluster]);
            for k in 0..four_sizes[cluster] {
                let id = format!("s{cluster}_{k}");
                // The secondary neutral is always grounded at the transformer.
                let grounded = k == 0 || (lv_four_wire + 1).is_multiple_of(secondary.ground_every);
                lv_four_wire += 1;
                builder.bus(id.clone(), four_wire(), grounded);
                if k > 0 {
                    let parent = if rng.chance(spec.chain_probability) {
                        k - 1
                    } else {
                        rng.below(k)
                    };
                    let length = rng.uniform(secondary.segment_m.0, secondary.segment_m.1);
                    builder.line(&members[parent], &id, four_wire(), "lv4", length);
                }
                four_wire_sites.push(LoadSite::FourWire { bus: id.clone() });
                members.push(id);
            }
            for k in 0..two_sizes[cluster] {
                let id = format!("d{cluster}_{k}");
                let phase = PHASES[drops % 3];
                drops += 1;
                builder.bus(id.clone(), two_wire(phase), false);
                let parent = &members[rng.below(members.len())];
                let length = rng.uniform(secondary.service_m.0, secondary.service_m.1);
                builder.line(parent, &id, two_wire(phase), "lv2", length);
                two_wire_sites.push(LoadSite::TwoWire { bus: id, phase });
            }
        }
    } else {
        for index in 0..spec.primary_buses {
            four_wire_sites.push(LoadSite::FourWire {
                bus: format!("p{index}"),
            });
        }
    }

    // Loads: every two-wire bus carries one single-phase load; three-phase
    // loads and the remaining single-phase loads take distinct four-wire
    // load buses.
    assert!(spec.single_phase_loads >= two_wire_sites.len());
    let four_wire_single = spec.single_phase_loads - two_wire_sites.len();
    assert!(spec.three_phase_loads + four_wire_single <= four_wire_sites.len());
    rng.shuffle(&mut four_wire_sites);
    four_wire_sites.truncate(spec.three_phase_loads + four_wire_single);
    let total_loads = spec.three_phase_loads + spec.single_phase_loads;
    let kinds = model_kinds(&mut rng, total_loads);
    // About 5% of constant-power loads omit v_nom, so the solver infers it
    // from the transformer winding (or nameplate) anchors of their zone.
    let mut constant_power: Vec<usize> = (0..total_loads)
        .filter(|&index| kinds[index] == ModelKind::ConstantPower)
        .collect();
    rng.shuffle(&mut constant_power);
    let omit_count = ((0.05 * constant_power.len() as f64).round() as usize)
        .max(1)
        .min(constant_power.len());
    let omitted: BTreeSet<usize> = constant_power.into_iter().take(omit_count).collect();
    let secondary_vll = spec.secondary.as_ref().map(|s| s.kv_ll * 1e3);
    let vll = secondary_vll.unwrap_or(primary_vll);
    let vln = vll / 3f64.sqrt();
    let mut single_phase_rotation = 0usize;

    let sites = four_wire_sites
        .iter()
        .take(spec.three_phase_loads)
        .map(|site| (site, true))
        .chain(
            four_wire_sites
                .iter()
                .skip(spec.three_phase_loads)
                .chain(two_wire_sites.iter())
                .map(|site| (site, false)),
        );
    for (index, ((site, three_phase), kind)) in sites.zip(kinds).enumerate() {
        let pf = rng.uniform(0.90, 0.98);
        let tan_phi = pf.acos().tan();
        let (bus, terminal_map, configuration, p_nom, v_nom) = if three_phase {
            let LoadSite::FourWire { bus } = site else {
                unreachable!("three-phase loads sit on four-wire buses")
            };
            let p: Vec<f64> = (0..3)
                .map(|_| {
                    1e3 * spec.load_scale
                        * rng.uniform(
                            spec.three_phase_kw_per_phase.0,
                            spec.three_phase_kw_per_phase.1,
                        )
                })
                .collect();
            if rng.chance(spec.wye_share) {
                (
                    bus.clone(),
                    four_wire(),
                    Configuration::Wye,
                    p,
                    vec![vln; 3],
                )
            } else {
                (
                    bus.clone(),
                    terminals(&PHASES),
                    Configuration::Delta,
                    p,
                    vec![vll; 3],
                )
            }
        } else {
            let (bus, phase) = match site {
                LoadSite::TwoWire { bus, phase } => (bus.clone(), *phase),
                LoadSite::FourWire { bus } => {
                    let phase = PHASES[single_phase_rotation % 3];
                    single_phase_rotation += 1;
                    (bus.clone(), phase)
                }
            };
            let p =
                1e3 * spec.load_scale * rng.uniform(spec.single_phase_kw.0, spec.single_phase_kw.1);
            (
                bus,
                two_wire(phase),
                Configuration::SinglePhase,
                vec![p],
                vec![vln],
            )
        };
        let branches = p_nom.len();
        let q_nom = p_nom.iter().map(|p| p * tan_phi).collect();
        let voltage_model = match kind {
            ModelKind::ConstantPower if omitted.contains(&index) => {
                DistLoadVoltageModel::ConstantPower { v_nom: Vec::new() }
            }
            ModelKind::ConstantPower => DistLoadVoltageModel::ConstantPower { v_nom },
            ModelKind::ConstantCurrent => DistLoadVoltageModel::ConstantCurrent { v_nom },
            ModelKind::ConstantImpedance => DistLoadVoltageModel::ConstantImpedance { v_nom },
            ModelKind::Zip => DistLoadVoltageModel::Zip {
                v_nom,
                alpha_z: vec![0.3; branches],
                alpha_i: vec![0.3; branches],
                alpha_p: vec![0.4; branches],
                beta_z: vec![0.4; branches],
                beta_i: vec![0.3; branches],
                beta_p: vec![0.3; branches],
            },
            ModelKind::Exponential => DistLoadVoltageModel::Exponential {
                v_nom,
                gamma_p: (0..branches).map(|_| rng.uniform(0.8, 1.6)).collect(),
                gamma_q: (0..branches).map(|_| rng.uniform(1.5, 3.0)).collect(),
            },
        };
        let mut load = DistLoad::new(
            format!("ld{index}"),
            bus,
            terminal_map,
            configuration,
            p_nom,
            q_nom,
        );
        load.voltage_model = voltage_model;
        builder.loads.push(load);
    }

    let mut network = MulticonductorNetwork::named(spec.name.clone());
    *network.base_frequency_mut() = spec.base_frequency;
    let mut codes = vec![
        // Primary: phase 0.25+j0.40, neutral 0.40+j0.40, mutual 0.05+j0.15 ohm/km.
        linecode("mv4", 4, (0.25, 0.40), (0.40, 0.40), (0.05, 0.15)),
    ];
    if spec.primary_taps > 0 {
        codes.push(linecode("mv2", 2, (0.35, 0.45), (0.40, 0.45), (0.05, 0.15)));
    }
    if spec.secondary.is_some() {
        // Low-voltage four-core cable and a two-core service cable.
        codes.push(linecode("lv4", 4, (0.32, 0.08), (0.32, 0.08), (0.05, 0.02)));
        codes.push(linecode("lv2", 2, (0.50, 0.09), (0.50, 0.09), (0.05, 0.03)));
    }
    *network.line_codes_mut() = codes;
    *network.buses_mut() = builder.buses;
    *network.lines_mut() = builder.lines;
    *network.transformers_mut() = builder.transformers;
    *network.loads_mut() = builder.loads;
    let source_vln = primary_vll / 3f64.sqrt();
    network.sources_mut().push(VoltageSource::new(
        "vs",
        "src",
        four_wire(),
        vec![source_vln, source_vln, source_vln, 0.0],
        vec![0.0, -2.0 * PI / 3.0, 2.0 * PI / 3.0, 0.0],
    ));
    network
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_matches_the_reference_stream() {
        // Reference values for seed 1234567 (Vigna's splitmix64.c).
        let mut rng = SplitMix64::new(1_234_567);
        assert_eq!(rng.next_u64(), 6_457_827_717_110_365_317);
        assert_eq!(rng.next_u64(), 3_203_168_211_198_807_973);
    }

    #[test]
    fn generation_is_deterministic() {
        let first = serde_json::to_string(&generate(Preset::Tiny)).unwrap();
        let second = serde_json::to_string(&generate(Preset::Tiny)).unwrap();
        assert_eq!(first, second);
        let feeder = serde_json::to_string(&generate(Preset::Feeder10k)).unwrap();
        assert_eq!(
            feeder,
            serde_json::to_string(&generate(Preset::Feeder10k)).unwrap()
        );
        assert_ne!(first, feeder);
    }

    #[test]
    fn every_preset_has_its_exact_target_shape() {
        for preset in Preset::ALL {
            assert_eq!(preset.spec().shape(), preset.target_shape(), "{preset}");
        }
        // Generation is cheap even in debug builds; only Tiny is solved.
        for preset in Preset::ALL {
            assert_eq!(
                FeederShape::of(&generate(preset)),
                preset.target_shape(),
                "{preset}"
            );
        }
        let ten = Preset::Feeder10k.target_shape();
        assert_eq!(
            (ten.buses, ten.terminals, ten.lines, ten.transformers),
            (9_950, 32_412, 9_299, 650)
        );
        assert_eq!((ten.loads, ten.load_branches), (7_721, 9_299));
        let large = Preset::Feeder106k.target_shape();
        assert_eq!(
            (large.buses, large.terminals, large.lines),
            (27_239, 106_038, 27_803)
        );
    }

    #[test]
    fn tiny_covers_every_load_type_and_voltage_model() {
        let network = generate(Preset::Tiny);
        let loads = network.loads();
        for configuration in [
            Configuration::Wye,
            Configuration::Delta,
            Configuration::SinglePhase,
        ] {
            assert!(loads.iter().any(|load| load.configuration == configuration));
        }
        let has = |test: fn(&DistLoadVoltageModel) -> bool| {
            loads.iter().any(|load| test(&load.voltage_model))
        };
        assert!(has(
            |m| matches!(m, DistLoadVoltageModel::ConstantPower { v_nom } if v_nom.is_empty())
        ));
        assert!(has(
            |m| matches!(m, DistLoadVoltageModel::ConstantPower { v_nom } if !v_nom.is_empty())
        ));
        assert!(has(|m| matches!(
            m,
            DistLoadVoltageModel::ConstantCurrent { .. }
        )));
        assert!(has(|m| matches!(
            m,
            DistLoadVoltageModel::ConstantImpedance { .. }
        )));
        assert!(has(|m| matches!(m, DistLoadVoltageModel::Zip { .. })));
        assert!(has(|m| matches!(
            m,
            DistLoadVoltageModel::Exponential { .. }
        )));
        assert!(network.switches().is_empty());
        assert!(network
            .buses()
            .iter()
            .all(|bus| bus.phase_indices(None).len() + 1 == bus.terminals.len()));
    }

    #[test]
    fn tiny_solves_with_the_default_options() {
        let instance =
            powerio_prob::McAcPfInstance::from_network(generate(Preset::Tiny)).expect("instance");
        let mut session = tellegen::McPfSession::new(instance, tellegen::McPfOptions::default())
            .expect("Tiny solves");
        let summary = session.summary().clone();
        assert!(summary.converged);
        assert_eq!(summary.factorization_count, 1);
        assert!(summary.matrix_dimension > 0 && summary.matrix_dimension < 198);
        let minimum = summary.min_voltage_pu.expect("load branches");
        assert!((0.90..=0.97).contains(&minimum), "minimum {minimum}");
        assert!(
            (4..=30).contains(&summary.iterations),
            "iterations {}",
            summary.iterations
        );
        // Every branch edited at once still reuses the factor and builds no
        // edited network.
        let edits: Vec<_> = session
            .load_branches()
            .into_iter()
            .map(|branch| tellegen::McLoadPowerEdit {
                load: branch.load,
                branch: branch.branch,
                p_w: branch.base_p_w * 1.05,
                q_var: branch.base_q_var * 1.05,
            })
            .collect();
        session
            .replace_load_powers(&edits)
            .expect("edited Tiny solves");
        assert_eq!(session.factorization_count(), 1);
        assert_eq!(session.materialization_count(), 0);
    }
}
