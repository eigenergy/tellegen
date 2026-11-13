//! Controllers defined at run time as block diagrams.
//!
//! A [`BlockGraph`] (JSON) wires blocks from the same library the compiled
//! models use (`blocks.rs`): lags with and without non-windup limits,
//! lead-lags, washouts, second-order filters, PI, integrators, sums, gains,
//! products, limits. It fills one controller slot of a generating unit
//! (exciter, governor or stabilizer) and reads that slot's ports. Compiled
//! once per session into an evaluation order and a state map, it behaves like
//! a compiled model: its states sit in the device's mass-matrix form (a zero
//! time constant makes a state algebraic), the Jacobian comes from the same
//! forward differences.
//!
//! Templates reproduce the compiled PSS/E models (`template`), so a user
//! starts from a validated model and changes it.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::blocks::*;

/// The controller slot a graph fills.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Slot {
    Exciter,
    Governor,
    Stabilizer,
}

impl Slot {
    /// Ports a graph of this slot can read, in evaluation order.
    #[must_use]
    pub fn ports(self) -> &'static [&'static str] {
        match self {
            // Terminal voltage, stabilizer signal, field current, voltage reference.
            Self::Exciter => &["vt", "vs", "xad_ifd", "v_ref"],
            // Speed and power reference (the output is the mechanical torque).
            Self::Governor => &["omega", "p_ref"],
            // Speed, electrical power (system base), terminal voltage,
            // electrical power on the machine base (`MBASE`), the input of
            // the PSS/E stabilizers' code 3.
            Self::Stabilizer => &["omega", "pe", "vt", "pe_m"],
        }
    }

    /// The reference the initialization solves for, if any.
    fn reference_port(self) -> Option<usize> {
        match self {
            Self::Exciter => Some(3),
            Self::Governor => Some(1),
            Self::Stabilizer => None,
        }
    }
}

fn yes() -> bool {
    true
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_true(b: &bool) -> bool {
    *b
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// One block. Time constants sit in the mass matrix; zero makes the state
/// algebraic.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    /// A port of the slot.
    Input {
        port: String,
    },
    Const {
        value: f64,
    },
    Gain {
        k: f64,
    },
    /// `Σ sᵢ uᵢ`, one sign per input.
    Sum {
        signs: Vec<f64>,
    },
    Product,
    /// `u₀ / u₁`.
    Divide,
    Clamp {
        min: f64,
        max: f64,
    },
    /// `clamp(u₀, u₁, u₂)`: limits from signals.
    ClampVar,
    /// The smallest input; with two inputs and `tie > 0`, the second wins
    /// unless the first is below it by more than `tie` (PSS/E GAST's load
    /// limit).
    Min {
        #[serde(default, skip_serializing_if = "is_zero")]
        tie: f64,
    },
    Max,
    /// The rectifier regulation `FEX(I_N)` of IEEE Std 421.5 AC exciters.
    Rectifier,
    /// Exciter saturation `S_E(u) = B (u − A)² / u` (0 without data).
    SatQuadratic {
        a: f64,
        b: f64,
    },
    /// `u₀` while `lo ≤ u₁ ≤ hi`, else 0; a zero limit bypasses (IEEE 421.5
    /// stabilizer output limiter).
    Gate {
        lo: f64,
        hi: f64,
    },
    /// `K/(1 + sT)`.
    Lag {
        k: f64,
        t: f64,
    },
    /// `K/(1 + sT)` with non-windup limits on the state. The output is the
    /// clamped state, or with `clamp_output: false` the state itself (PSS/E
    /// SEXS: the state can pass a limit within a step).
    LagLimited {
        k: f64,
        t: f64,
        min: f64,
        max: f64,
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        clamp_output: bool,
    },
    /// `K (1 + sT1)/(1 + sT2)`.
    LeadLag {
        k: f64,
        t1: f64,
        t2: f64,
    },
    /// `K s/(1 + sT)` (a unity-gain washout has `K = T`).
    Washout {
        k: f64,
        t: f64,
    },
    /// `K/s`.
    Integrator {
        k: f64,
    },
    /// `clamp(Kp u + Ki x)`, `ẋ = u` only while unclamped, and with `rate`
    /// the derivative limited to `±rate` (PSS/E HYGOV's gate velocity).
    PiLimited {
        kp: f64,
        ki: f64,
        min: f64,
        max: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rate: Option<f64>,
    },
    /// `K/(1 + sT1 + s²T2)`.
    Lag2 {
        k: f64,
        t1: f64,
        t2: f64,
    },
    /// `(1 + sT3 + s²T4)/(1 + sT1 + s²T2)`.
    LeadLag2 {
        t1: f64,
        t2: f64,
        t3: f64,
        t4: f64,
    },
}

impl Block {
    fn n_states(&self) -> usize {
        match self {
            Self::Lag { .. }
            | Self::LagLimited { .. }
            | Self::LeadLag { .. }
            | Self::Washout { .. }
            | Self::Integrator { .. }
            | Self::PiLimited { .. } => 1,
            Self::Lag2 { .. } | Self::LeadLag2 { .. } => 2,
            _ => 0,
        }
    }

    /// Inputs the block takes (`None`: any number ≥ 1).
    fn arity(&self) -> Option<usize> {
        match self {
            Self::Input { .. } | Self::Const { .. } => Some(0),
            Self::Sum { signs } => Some(signs.len()),
            Self::Product | Self::Min { .. } | Self::Max => None,
            Self::Divide | Self::Gate { .. } => Some(2),
            Self::ClampVar => Some(3),
            _ => Some(1),
        }
    }

    /// The output depends on the states only, not on this instant's inputs:
    /// such a block breaks an algebraic loop.
    fn state_output(&self) -> bool {
        matches!(
            self,
            Self::Lag { .. }
                | Self::LagLimited { .. }
                | Self::Integrator { .. }
                | Self::Lag2 { .. }
        )
    }

    fn mass(&self, m: &mut [f64]) {
        match self {
            Self::Lag { t, .. } | Self::LagLimited { t, .. } | Self::Washout { t, .. } => m[0] = *t,
            Self::LeadLag { t2, .. } => m[0] = *t2,
            Self::Integrator { .. } | Self::PiLimited { .. } => m[0] = 1.0,
            Self::Lag2 { t2, .. } | Self::LeadLag2 { t2, .. } => {
                m[0] = *t2;
                m[1] = 1.0;
            }
            _ => {}
        }
    }

    fn check(&self) -> Result<(), String> {
        let finite = |v: &[f64]| v.iter().all(|x| x.is_finite());
        let ok = match self {
            Self::Const { value } => finite(&[*value]),
            Self::Gain { k } | Self::Integrator { k } => finite(&[*k]),
            Self::Sum { signs } => !signs.is_empty() && finite(signs),
            Self::Clamp { min, max } => finite(&[*min, *max]) && min <= max,
            Self::Gate { lo, hi } => finite(&[*lo, *hi]),
            Self::Lag { k, t } | Self::Washout { k, t } => finite(&[*k, *t]) && *t >= 0.0,
            Self::LagLimited { k, t, min, max, .. } => {
                finite(&[*k, *t, *min, *max]) && *t >= 0.0 && min <= max
            }
            Self::LeadLag { k, t1, t2 } => finite(&[*k, *t1, *t2]) && *t2 >= 0.0,
            Self::PiLimited {
                kp,
                ki,
                min,
                max,
                rate,
            } => {
                finite(&[*kp, *ki, *min, *max, rate.unwrap_or(0.0)])
                    && min <= max
                    && rate.is_none_or(|r| r >= 0.0)
            }
            Self::Min { tie } => finite(&[*tie]) && *tie >= 0.0,
            Self::SatQuadratic { a, b } => finite(&[*a, *b]),
            Self::Lag2 { k, t1, t2 } => finite(&[*k, *t1, *t2]) && *t2 >= 0.0,
            Self::LeadLag2 { t1, t2, t3, t4 } => finite(&[*t1, *t2, *t3, *t4]) && *t2 >= 0.0,
            _ => true,
        };
        if ok {
            Ok(())
        } else {
            Err("parameters must be finite, time constants ≥ 0, min ≤ max".into())
        }
    }

    /// Output of a state-output block from its states.
    #[cfg(test)]
    fn output_from_states(&self, x: &[f64]) -> f64 {
        match self {
            Self::LagLimited {
                min,
                max,
                clamp_output: true,
                ..
            } => x[0].clamp(*min, *max),
            Self::Lag2 { .. } => x[1],
            _ => x[0],
        }
    }

    /// Output and state derivatives (right-hand sides of the mass-matrix form).
    fn eval(&self, u: &[f64], port: f64, x: &[f64], dx: &mut [f64]) -> f64 {
        match self {
            Self::Input { .. } => port,
            Self::Const { value } => *value,
            Self::Gain { k } => k * u[0],
            Self::Sum { signs } => signs.iter().zip(u).map(|(s, v)| s * v).sum(),
            Self::Product => u.iter().product(),
            Self::Divide => u[0] / u[1],
            Self::Clamp { min, max } => u[0].clamp(*min, *max),
            Self::ClampVar => u[0].max(u[1]).min(u[2]),
            Self::Min { tie } if *tie > 0.0 && u.len() == 2 => {
                if u[0] < u[1] - tie {
                    u[0]
                } else {
                    u[1]
                }
            }
            Self::Min { .. } => u.iter().copied().fold(f64::INFINITY, f64::min),
            Self::Rectifier => rectifier(u[0]),
            Self::SatQuadratic { a, b } => {
                if *b == 0.0 {
                    0.0
                } else {
                    b * (u[0] - a).powi(2) / u[0]
                }
            }
            Self::Max => u.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            Self::Gate { lo, hi } => {
                if *lo == 0.0 || *hi == 0.0 || (*lo <= u[1] && u[1] <= *hi) {
                    u[0]
                } else {
                    0.0
                }
            }
            Self::Lag { k, .. } => {
                let (y, d) = low_pass_mm(u[0], x[0], *k);
                dx[0] = d;
                y
            }
            Self::LagLimited {
                k,
                min,
                max,
                clamp_output,
                ..
            } => {
                let (y, d) = low_pass_nonwindup_mm(u[0], x[0], *k, *min, *max);
                dx[0] = d;
                if *clamp_output {
                    y
                } else {
                    x[0]
                }
            }
            Self::LeadLag { k, t1, t2 } => {
                let (y, d) = lead_lag_mm(u[0], x[0], *k, *t1, *t2);
                dx[0] = d;
                y
            }
            Self::Washout { k, t } => {
                let (y, d) = high_pass_mm(u[0], x[0], *k, *t);
                dx[0] = d;
                y
            }
            Self::Integrator { k } => {
                dx[0] = k * u[0];
                x[0]
            }
            Self::PiLimited {
                kp,
                ki,
                min,
                max,
                rate,
            } => {
                let (y, d) = pi_nonwindup(u[0], x[0], *kp, *ki, *min, *max);
                dx[0] = rate.map_or(d, |r| d.clamp(-r, r));
                y
            }
            Self::Lag2 { k, t1, .. } => {
                let (y, d1, d2) = low_pass_2nd_mm(u[0], x[0], x[1], *k, *t1);
                dx[0] = d1;
                dx[1] = d2;
                y
            }
            Self::LeadLag2 { t1, t2, t3, t4 } => {
                let (y, d1, d2) = lead_lag_2nd_mm(u[0], x[0], x[1], *t1, *t2, *t3, *t4);
                dx[0] = d1;
                dx[1] = d2;
                y
            }
        }
    }

    /// Steady-state output and states for input `u` (initial guess only;
    /// an integrator's state is unknown and starts at its current value).
    /// Limits are left out: the guess must move with the reference so the
    /// secant search finds it; Newton then solves the limited model.
    fn steady(&self, u: &[f64], port: f64, x: &mut [f64]) -> f64 {
        match self {
            Self::Lag { k, .. } | Self::LagLimited { k, .. } => {
                x[0] = k * u[0];
                x[0]
            }
            Self::Clamp { .. } | Self::ClampVar | Self::Gate { .. } => u[0],
            Self::LeadLag { k, t1, t2 } => {
                let r = if *t2 < f64::EPSILON { 0.0 } else { t1 / t2 };
                x[0] = k * (1.0 - r) * u[0];
                k * u[0]
            }
            Self::Washout { k, t } => {
                x[0] = if *t < f64::EPSILON {
                    0.0
                } else {
                    -(k / t) * u[0]
                };
                0.0
            }
            Self::Integrator { .. } => x[0],
            Self::PiLimited { ki, .. } => ki * x[0],
            Self::Lag2 { k, .. } => {
                x[0] = 0.0;
                x[1] = k * u[0];
                x[1]
            }
            Self::LeadLag2 { .. } => {
                x[0] = 0.0;
                x[1] = u[0];
                u[0]
            }
            _ => self.eval(u, port, x, &mut [0.0; 2]),
        }
    }
}

/// One node: an id, a block, its input node ids (in the block's order).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    #[serde(flatten)]
    pub block: Block,
    #[serde(default)]
    pub inputs: Vec<String>,
    /// Names of the block's states (default: the node id, `.1`/`.2` for two).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<String>,
    /// Starting value `[a, b]` of an integrating state (integrator, PI) for
    /// the initialization: `a · target + b`, with the slot's output at rest
    /// as the target (default `[1, 0]`). Picks the physical branch where the
    /// steady state has more than one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<[f64; 2]>,
    /// Position in the editor (ignored by the engine).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<[f64; 2]>,
}

/// A controller as a block diagram.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlockGraph {
    pub slot: Slot,
    #[serde(default)]
    pub name: String,
    pub nodes: Vec<GraphNode>,
    /// The node whose output is the slot's output (field voltage, torque,
    /// stabilizer signal).
    pub output: String,
}

/// A graph checked and ordered for evaluation.
#[derive(Clone, Debug)]
pub(crate) struct CompiledGraph {
    pub graph: BlockGraph,
    blocks: Vec<Block>,
    inputs: Vec<Vec<usize>>,
    /// Port index of each `Input` node.
    port: Vec<usize>,
    /// First state of each node.
    offset: Vec<usize>,
    /// Nodes whose output needs this instant's inputs, in dependency order.
    order: Vec<usize>,
    output: usize,
    n_states: usize,
    names: Vec<String>,
    mass: Vec<f64>,
    max_inputs: usize,
    /// The evaluation as a flat list of operations ([`Op`]).
    tape: Vec<Op>,
    /// The terms of the tape's sums: sign and input node.
    terms: Vec<(f64, u32)>,
    /// The input nodes of the tape's products, minima and maxima.
    args: Vec<u32>,
}

/// Graphs up to this many nodes evaluate on the stack.
const STACK_NODES: usize = 64;

/// One operation of a compiled graph: a block with the places of its inputs
/// (`a`, `b`, …: node outputs), its output (`out`) and its first state (`x`)
/// resolved. A graph evaluates as one pass over its list, in three parts:
/// the outputs that are states, the blocks that need this instant's inputs
/// in dependency order, the derivatives of the blocks whose output is a
/// state. Each operation computes what [`Block::eval`] does, with the same
/// expressions, so the results are the same bits; a model evaluates a few
/// thousand of them per unit and step, and finding every block's inputs and
/// states through the graph's lists each time took longer than the blocks.
#[derive(Clone, Debug)]
enum Op {
    State {
        out: u32,
        x: u32,
    },
    StateClamped {
        out: u32,
        x: u32,
        min: f64,
        max: f64,
    },
    Input {
        out: u32,
        port: u32,
    },
    Const {
        out: u32,
        value: f64,
    },
    Gain {
        out: u32,
        a: u32,
        k: f64,
    },
    /// `terms[first..first + len]`.
    Sum {
        out: u32,
        first: u32,
        len: u32,
    },
    /// `args[first..first + len]`, as for `Min` and `Max`.
    Product {
        out: u32,
        first: u32,
        len: u32,
    },
    Divide {
        out: u32,
        a: u32,
        b: u32,
    },
    Clamp {
        out: u32,
        a: u32,
        min: f64,
        max: f64,
    },
    ClampVar {
        out: u32,
        a: u32,
        lo: u32,
        hi: u32,
    },
    MinTie {
        out: u32,
        a: u32,
        b: u32,
        tie: f64,
    },
    Min {
        out: u32,
        first: u32,
        len: u32,
    },
    Max {
        out: u32,
        first: u32,
        len: u32,
    },
    Rectifier {
        out: u32,
        a: u32,
    },
    SatQuadratic {
        out: u32,
        a: u32,
        sa: f64,
        sb: f64,
    },
    Gate {
        out: u32,
        a: u32,
        b: u32,
        lo: f64,
        hi: f64,
    },
    LeadLag {
        out: u32,
        a: u32,
        x: u32,
        k: f64,
        t1: f64,
        t2: f64,
    },
    Washout {
        out: u32,
        a: u32,
        x: u32,
        k: f64,
        t: f64,
    },
    PiLimited {
        out: u32,
        a: u32,
        x: u32,
        kp: f64,
        ki: f64,
        min: f64,
        max: f64,
        rate: Option<f64>,
    },
    LeadLag2 {
        out: u32,
        a: u32,
        x: u32,
        t1: f64,
        t2: f64,
        t3: f64,
        t4: f64,
    },
    LagRate {
        a: u32,
        x: u32,
        k: f64,
    },
    LagLimitedRate {
        a: u32,
        x: u32,
        k: f64,
        min: f64,
        max: f64,
    },
    IntegratorRate {
        a: u32,
        x: u32,
        k: f64,
    },
    Lag2Rate {
        a: u32,
        x: u32,
        k: f64,
        t1: f64,
    },
}

impl CompiledGraph {
    /// Check and order a graph.
    ///
    /// # Errors
    /// Unknown or duplicate ids, a port the slot does not have, a wrong
    /// number of inputs, invalid parameters, or an algebraic loop (a cycle
    /// without a block whose output is a state).
    pub(crate) fn new(graph: &BlockGraph) -> Result<Self, String> {
        let n = graph.nodes.len();
        let mut index = HashMap::new();
        for (i, node) in graph.nodes.iter().enumerate() {
            if node.id.trim().is_empty() {
                return Err(format!("node {i}: empty id"));
            }
            if index.insert(node.id.as_str(), i).is_some() {
                return Err(format!("node id {:?} used twice", node.id));
            }
        }
        let ports = graph.slot.ports();
        let mut inputs = Vec::with_capacity(n);
        let mut port = vec![usize::MAX; n];
        let mut offset = Vec::with_capacity(n);
        let mut names = Vec::new();
        let mut mass = Vec::new();
        for (i, node) in graph.nodes.iter().enumerate() {
            let at = |m: String| format!("node {:?}: {m}", node.id);
            node.block.check().map_err(at)?;
            if let Some(a) = node.block.arity() {
                if node.inputs.len() != a {
                    return Err(at(format!("takes {a} input(s), has {}", node.inputs.len())));
                }
            } else if node.inputs.is_empty() {
                return Err(at("takes at least one input".into()));
            }
            if let Block::Input { port: p } = &node.block {
                port[i] = ports.iter().position(|q| q == p).ok_or_else(|| {
                    at(format!(
                        "the {:?} slot has no port {p:?} (ports: {})",
                        graph.slot,
                        ports.join(", ")
                    ))
                })?;
            }
            inputs.push(
                node.inputs
                    .iter()
                    .enumerate()
                    .map(|(k, s)| {
                        if s.trim().is_empty() {
                            return Err(at(format!("input {} is not connected", k + 1)));
                        }
                        index
                            .get(s.as_str())
                            .copied()
                            .ok_or_else(|| at(format!("input {} names no block {s:?}", k + 1)))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
            let k = node.block.n_states();
            offset.push(mass.len());
            let mut m = vec![1.0; k];
            node.block.mass(&mut m);
            mass.extend(m);
            for j in 0..k {
                names.push(match (node.states.get(j), k) {
                    (Some(s), _) => s.clone(),
                    (None, 1) => node.id.clone(),
                    (None, _) => format!("{}.{}", node.id, j + 1),
                });
            }
        }
        let output = *index
            .get(graph.output.as_str())
            .ok_or_else(|| format!("output {:?} names no node", graph.output))?;
        // Order the nodes that need this instant's inputs (depth-first,
        // edges into state-output blocks cut).
        let mut state = vec![0u8; n];
        let mut order = Vec::new();
        fn visit(
            i: usize,
            blocks: &[&Block],
            inputs: &[Vec<usize>],
            ids: &[&str],
            state: &mut [u8],
            order: &mut Vec<usize>,
        ) -> Result<(), String> {
            match state[i] {
                2 => return Ok(()),
                1 => {
                    return Err(format!(
                        "algebraic loop through node {:?}: put a lag or integrator in the loop",
                        ids[i]
                    ))
                }
                _ => {}
            }
            state[i] = 1;
            if !blocks[i].state_output() {
                for &j in &inputs[i] {
                    if !blocks[j].state_output() {
                        visit(j, blocks, inputs, ids, state, order)?;
                    }
                }
            }
            state[i] = 2;
            if !blocks[i].state_output() {
                order.push(i);
            }
            Ok(())
        }
        let block_refs: Vec<&Block> = graph.nodes.iter().map(|g| &g.block).collect();
        let ids: Vec<&str> = graph.nodes.iter().map(|g| g.id.as_str()).collect();
        for i in 0..n {
            visit(i, &block_refs, &inputs, &ids, &mut state, &mut order)?;
        }
        let max_inputs = inputs.iter().map(Vec::len).max().unwrap_or(0);
        let mut compiled = Self {
            graph: graph.clone(),
            blocks: graph.nodes.iter().map(|g| g.block.clone()).collect(),
            inputs,
            port,
            offset,
            order,
            output,
            n_states: mass.len(),
            names,
            mass,
            max_inputs,
            tape: Vec::new(),
            terms: Vec::new(),
            args: Vec::new(),
        };
        compiled.compile()?;
        Ok(compiled)
    }

    /// Build the operation list ([`Op`]) from the blocks, their wiring and
    /// the evaluation order.
    fn compile(&mut self) -> Result<(), String> {
        let at = |i: usize| u32::try_from(i).map_err(|_| "the graph is too large".to_owned());
        let n = self.blocks.len();
        let mut tape = Vec::with_capacity(2 * n);
        let (mut terms, mut args) = (Vec::new(), Vec::new());
        // The outputs that are states.
        for i in 0..n {
            let (out, x) = (at(i)?, at(self.offset[i])?);
            match &self.blocks[i] {
                Block::LagLimited {
                    min,
                    max,
                    clamp_output: true,
                    ..
                } => tape.push(Op::StateClamped {
                    out,
                    x,
                    min: *min,
                    max: *max,
                }),
                Block::Lag2 { .. } => tape.push(Op::State { out, x: x + 1 }),
                b if b.state_output() => tape.push(Op::State { out, x }),
                _ => {}
            }
        }
        // The blocks that need this instant's inputs, in dependency order.
        for &i in &self.order {
            let (out, x) = (at(i)?, at(self.offset[i])?);
            let ins = &self.inputs[i];
            let input = |k: usize| at(ins[k]);
            let list = |args: &mut Vec<u32>| -> Result<(u32, u32), String> {
                let first = at(args.len())?;
                for &j in ins {
                    args.push(at(j)?);
                }
                Ok((first, at(ins.len())?))
            };
            tape.push(match &self.blocks[i] {
                Block::Input { .. } => Op::Input {
                    out,
                    // A port the slot lacks reads as zero.
                    port: u32::try_from(self.port[i]).unwrap_or(u32::MAX),
                },
                Block::Const { value } => Op::Const { out, value: *value },
                Block::Gain { k } => Op::Gain {
                    out,
                    a: input(0)?,
                    k: *k,
                },
                Block::Sum { signs } => {
                    let first = at(terms.len())?;
                    for (s, &j) in signs.iter().zip(ins) {
                        terms.push((*s, at(j)?));
                    }
                    Op::Sum {
                        out,
                        first,
                        len: at(signs.len().min(ins.len()))?,
                    }
                }
                Block::Product => {
                    let (first, len) = list(&mut args)?;
                    Op::Product { out, first, len }
                }
                Block::Divide => Op::Divide {
                    out,
                    a: input(0)?,
                    b: input(1)?,
                },
                Block::Clamp { min, max } => Op::Clamp {
                    out,
                    a: input(0)?,
                    min: *min,
                    max: *max,
                },
                Block::ClampVar => Op::ClampVar {
                    out,
                    a: input(0)?,
                    lo: input(1)?,
                    hi: input(2)?,
                },
                Block::Min { tie } if *tie > 0.0 && ins.len() == 2 => Op::MinTie {
                    out,
                    a: input(0)?,
                    b: input(1)?,
                    tie: *tie,
                },
                Block::Min { .. } => {
                    let (first, len) = list(&mut args)?;
                    Op::Min { out, first, len }
                }
                Block::Max => {
                    let (first, len) = list(&mut args)?;
                    Op::Max { out, first, len }
                }
                Block::Rectifier => Op::Rectifier { out, a: input(0)? },
                Block::SatQuadratic { a, b } => Op::SatQuadratic {
                    out,
                    a: input(0)?,
                    sa: *a,
                    sb: *b,
                },
                Block::Gate { lo, hi } => Op::Gate {
                    out,
                    a: input(0)?,
                    b: input(1)?,
                    lo: *lo,
                    hi: *hi,
                },
                Block::LeadLag { k, t1, t2 } => Op::LeadLag {
                    out,
                    a: input(0)?,
                    x,
                    k: *k,
                    t1: *t1,
                    t2: *t2,
                },
                Block::Washout { k, t } => Op::Washout {
                    out,
                    a: input(0)?,
                    x,
                    k: *k,
                    t: *t,
                },
                Block::PiLimited {
                    kp,
                    ki,
                    min,
                    max,
                    rate,
                } => Op::PiLimited {
                    out,
                    a: input(0)?,
                    x,
                    kp: *kp,
                    ki: *ki,
                    min: *min,
                    max: *max,
                    rate: *rate,
                },
                Block::LeadLag2 { t1, t2, t3, t4 } => Op::LeadLag2 {
                    out,
                    a: input(0)?,
                    x,
                    t1: *t1,
                    t2: *t2,
                    t3: *t3,
                    t4: *t4,
                },
                Block::Lag { .. }
                | Block::LagLimited { .. }
                | Block::Integrator { .. }
                | Block::Lag2 { .. } => {
                    return Err("a block whose output is a state is not ordered".into())
                }
            });
        }
        // The derivatives of the blocks whose output is a state.
        for i in 0..n {
            let x = at(self.offset[i])?;
            let a = || at(self.inputs[i][0]);
            match &self.blocks[i] {
                Block::Lag { k, .. } => tape.push(Op::LagRate { a: a()?, x, k: *k }),
                Block::LagLimited { k, min, max, .. } => tape.push(Op::LagLimitedRate {
                    a: a()?,
                    x,
                    k: *k,
                    min: *min,
                    max: *max,
                }),
                Block::Integrator { k } => tape.push(Op::IntegratorRate { a: a()?, x, k: *k }),
                Block::Lag2 { k, t1, .. } => tape.push(Op::Lag2Rate {
                    a: a()?,
                    x,
                    k: *k,
                    t1: *t1,
                }),
                _ => {}
            }
        }
        self.tape = tape;
        self.terms = terms;
        self.args = args;
        Ok(())
    }

    pub(crate) fn n_states(&self) -> usize {
        self.n_states
    }

    pub(crate) fn state_names(&self) -> &[String] {
        &self.names
    }

    pub(crate) fn mass(&self, m: &mut [f64]) {
        m.copy_from_slice(&self.mass);
    }

    #[cfg(test)]
    fn states<'a>(&self, i: usize, x: &'a [f64]) -> &'a [f64] {
        &x[self.offset[i]..self.offset[i] + self.blocks[i].n_states()]
    }

    /// Slot output and state derivatives for the port values `ports`.
    pub(crate) fn eval(&self, x: &[f64], ports: &[f64], f: &mut [f64]) -> f64 {
        let n = self.blocks.len();
        if n <= STACK_NODES {
            let mut y = [0.0; STACK_NODES];
            self.run(x, ports, f, &mut y[..n]);
            y[self.output]
        } else {
            self.eval_all(x, ports, f)[self.output]
        }
    }

    /// Every node's output, in node order (the signals a diagram shows).
    pub(crate) fn signals(&self, x: &[f64], ports: &[f64]) -> Vec<f64> {
        let mut f = vec![0.0; self.n_states];
        self.eval_all(x, ports, &mut f)
    }

    /// Node ids, in node order.
    pub(crate) fn node_ids(&self) -> impl Iterator<Item = &str> {
        self.graph.nodes.iter().map(|n| n.id.as_str())
    }

    fn eval_all(&self, x: &[f64], ports: &[f64], f: &mut [f64]) -> Vec<f64> {
        let mut y = vec![0.0; self.blocks.len()];
        self.run(x, ports, f, &mut y);
        y
    }

    /// Every node's output into `y` (one per node) and the state
    /// derivatives into `f`: the operation list, once through.
    fn run(&self, x: &[f64], ports: &[f64], f: &mut [f64], y: &mut [f64]) {
        let (terms, args) = (self.terms.as_slice(), self.args.as_slice());
        let span = |first: u32, len: u32| first as usize..(first + len) as usize;
        for op in &self.tape {
            match *op {
                Op::State { out, x: s } => y[out as usize] = x[s as usize],
                Op::StateClamped {
                    out,
                    x: s,
                    min,
                    max,
                } => y[out as usize] = x[s as usize].clamp(min, max),
                Op::Input { out, port } => {
                    y[out as usize] = ports.get(port as usize).copied().unwrap_or(0.0);
                }
                Op::Const { out, value } => y[out as usize] = value,
                Op::Gain { out, a, k } => y[out as usize] = k * y[a as usize],
                Op::Sum { out, first, len } => {
                    y[out as usize] = terms[span(first, len)]
                        .iter()
                        .map(|&(s, a)| s * y[a as usize])
                        .sum();
                }
                Op::Product { out, first, len } => {
                    y[out as usize] = args[span(first, len)]
                        .iter()
                        .map(|&a| y[a as usize])
                        .product();
                }
                Op::Divide { out, a, b } => y[out as usize] = y[a as usize] / y[b as usize],
                Op::Clamp { out, a, min, max } => y[out as usize] = y[a as usize].clamp(min, max),
                Op::ClampVar { out, a, lo, hi } => {
                    y[out as usize] = y[a as usize].max(y[lo as usize]).min(y[hi as usize]);
                }
                Op::MinTie { out, a, b, tie } => {
                    let (u0, u1) = (y[a as usize], y[b as usize]);
                    y[out as usize] = if u0 < u1 - tie { u0 } else { u1 };
                }
                Op::Min { out, first, len } => {
                    y[out as usize] = args[span(first, len)]
                        .iter()
                        .map(|&a| y[a as usize])
                        .fold(f64::INFINITY, f64::min);
                }
                Op::Max { out, first, len } => {
                    y[out as usize] = args[span(first, len)]
                        .iter()
                        .map(|&a| y[a as usize])
                        .fold(f64::NEG_INFINITY, f64::max);
                }
                Op::Rectifier { out, a } => y[out as usize] = rectifier(y[a as usize]),
                Op::SatQuadratic { out, a, sa, sb } => {
                    let u = y[a as usize];
                    y[out as usize] = if sb == 0.0 {
                        0.0
                    } else {
                        sb * (u - sa).powi(2) / u
                    };
                }
                Op::Gate { out, a, b, lo, hi } => {
                    let u1 = y[b as usize];
                    y[out as usize] = if lo == 0.0 || hi == 0.0 || (lo <= u1 && u1 <= hi) {
                        y[a as usize]
                    } else {
                        0.0
                    };
                }
                Op::LeadLag {
                    out,
                    a,
                    x: s,
                    k,
                    t1,
                    t2,
                } => {
                    let (v, d) = lead_lag_mm(y[a as usize], x[s as usize], k, t1, t2);
                    f[s as usize] = d;
                    y[out as usize] = v;
                }
                Op::Washout { out, a, x: s, k, t } => {
                    let (v, d) = high_pass_mm(y[a as usize], x[s as usize], k, t);
                    f[s as usize] = d;
                    y[out as usize] = v;
                }
                Op::PiLimited {
                    out,
                    a,
                    x: s,
                    kp,
                    ki,
                    min,
                    max,
                    rate,
                } => {
                    let (v, d) = pi_nonwindup(y[a as usize], x[s as usize], kp, ki, min, max);
                    f[s as usize] = rate.map_or(d, |r| d.clamp(-r, r));
                    y[out as usize] = v;
                }
                Op::LeadLag2 {
                    out,
                    a,
                    x: s,
                    t1,
                    t2,
                    t3,
                    t4,
                } => {
                    let s = s as usize;
                    let (v, d1, d2) =
                        lead_lag_2nd_mm(y[a as usize], x[s], x[s + 1], t1, t2, t3, t4);
                    f[s] = d1;
                    f[s + 1] = d2;
                    y[out as usize] = v;
                }
                Op::LagRate { a, x: s, k } => {
                    f[s as usize] = low_pass_mm(y[a as usize], x[s as usize], k).1;
                }
                Op::LagLimitedRate {
                    a,
                    x: s,
                    k,
                    min,
                    max,
                } => {
                    f[s as usize] =
                        low_pass_nonwindup_mm(y[a as usize], x[s as usize], k, min, max).1;
                }
                Op::IntegratorRate { a, x: s, k } => f[s as usize] = k * y[a as usize],
                Op::Lag2Rate { a, x: s, k, t1 } => {
                    let s = s as usize;
                    let (_, d1, d2) = low_pass_2nd_mm(y[a as usize], x[s], x[s + 1], k, t1);
                    f[s] = d1;
                    f[s + 1] = d2;
                }
            }
        }
    }

    /// [`Self::run`] block by block through the graph's lists, as the
    /// blocks define it ([`Block::eval`]): what the operation list must
    /// reproduce to the bit.
    #[cfg(test)]
    fn run_blocks(&self, x: &[f64], ports: &[f64], f: &mut [f64], y: &mut [f64]) {
        let n = self.blocks.len();
        let mut u = vec![0.0; self.max_inputs];
        for i in 0..n {
            if self.blocks[i].state_output() {
                y[i] = self.blocks[i].output_from_states(self.states(i, x));
            }
        }
        let mut scratch = [0.0; 2];
        for &i in &self.order {
            let k = self.inputs[i].len();
            for (j, &s) in self.inputs[i].iter().enumerate() {
                u[j] = y[s];
            }
            let port = ports.get(self.port[i]).copied().unwrap_or(0.0);
            let nk = self.blocks[i].n_states();
            let o = self.offset[i];
            let xs = &x[o..o + nk];
            y[i] = self.blocks[i].eval(&u[..k], port, xs, &mut scratch[..nk]);
            f[o..o + nk].copy_from_slice(&scratch[..nk]);
        }
        for i in 0..n {
            if self.blocks[i].state_output() {
                let k = self.inputs[i].len();
                for (j, &s) in self.inputs[i].iter().enumerate() {
                    u[j] = y[s];
                }
                let nk = self.blocks[i].n_states();
                let o = self.offset[i];
                self.blocks[i].eval(&u[..k], 0.0, &x[o..o + nk], &mut scratch[..nk]);
                f[o..o + nk].copy_from_slice(&scratch[..nk]);
            }
        }
    }

    /// Steady-state states and output for the port values (initial guess):
    /// the blocks at rest, swept until the loop settles. Integrating states
    /// (integrators, PI) keep the values in `x`. Also returns each node's
    /// first input, which an integrating block needs at zero.
    fn steady(&self, ports: &[f64], x: &mut [f64]) -> (f64, Vec<f64>) {
        let n = self.blocks.len();
        let mut y = vec![0.0; n];
        let mut first = vec![0.0; n];
        let mut u = vec![0.0; self.max_inputs];
        for _ in 0..50 {
            let mut change = 0.0_f64;
            for i in 0..n {
                let k = self.inputs[i].len();
                for (j, &s) in self.inputs[i].iter().enumerate() {
                    u[j] = y[s];
                }
                first[i] = if k > 0 { u[0] } else { 0.0 };
                let port = ports.get(self.port[i]).copied().unwrap_or(0.0);
                let o = self.offset[i];
                let nk = self.blocks[i].n_states();
                let v = self.blocks[i].steady(&u[..k], port, &mut x[o..o + nk]);
                if v.is_finite() {
                    change = change.max((v - y[i]).abs());
                }
                y[i] = v;
            }
            if change < 1e-12 {
                break;
            }
        }
        (y[self.output], first)
    }

    /// Initial states and reference. The unknowns are the reference (if the
    /// slot has one) and the integrating states; at rest the output equals
    /// `target` and every integrating block's input is zero. Solved by Newton
    /// on the steady-state map (limits left out); the device's own Newton
    /// then solves the limited model from here. `ports` holds the operating
    /// point; its reference entry is replaced.
    pub(crate) fn initial_guess(&self, ports: &[f64], target: f64) -> (Vec<f64>, f64) {
        let r = self.graph.slot.reference_port();
        let integ: Vec<(usize, usize)> = (0..self.blocks.len())
            .filter(|&i| {
                matches!(
                    self.blocks[i],
                    Block::Integrator { .. } | Block::PiLimited { .. }
                )
            })
            .map(|i| (i, self.offset[i]))
            .collect();
        let shift = usize::from(r.is_some());
        let nz = shift + integ.len();
        let mut x = vec![0.0; self.n_states];
        if nz == 0 {
            self.steady(ports, &mut x);
            return (x, 0.0);
        }
        let residual = |z: &[f64], x: &mut Vec<f64>| -> Vec<f64> {
            let mut p = ports.to_vec();
            x.iter_mut().for_each(|v| *v = 0.0);
            if let Some(r) = r {
                p[r] = z[0];
            }
            for (j, &(_, o)) in integ.iter().enumerate() {
                x[o] = z[shift + j];
            }
            let (out, first) = self.steady(&p, x);
            let mut g = Vec::with_capacity(nz);
            if r.is_some() {
                g.push(out - target);
            }
            g.extend(integ.iter().map(|&(i, _)| first[i]));
            g
        };
        let default = if target.is_finite() && target != 0.0 {
            target
        } else {
            1.0
        };
        let mut z: Vec<f64> = r
            .map(|r| ports[r])
            .into_iter()
            .chain(integ.iter().map(|&(i, _)| {
                self.graph.nodes[i]
                    .start
                    .map_or(default, |[a, b]| a * target + b)
            }))
            .collect();
        let norm = |g: &[f64]| g.iter().fold(0.0_f64, |a, v| a.max(v.abs()));
        let mut g = residual(&z, &mut x);
        for _ in 0..50 {
            let g0 = norm(&g);
            if !(g0 > 1e-12) {
                break;
            }
            // Forward-difference Jacobian and a damped Newton step.
            let mut jac = vec![0.0; nz * nz];
            for c in 0..nz {
                let h = 1e-7 * z[c].abs().max(1.0);
                let keep = z[c];
                z[c] = keep + h;
                let g1 = residual(&z, &mut x);
                z[c] = keep;
                for row in 0..nz {
                    jac[row * nz + c] = (g1[row] - g[row]) / h;
                }
            }
            // A singular map (an output that does not depend on the
            // reference, as GAST at its load limit) takes the regularized
            // least-squares step, which leaves such an unknown where it is.
            let Some(dz) = solve_small(jac.clone(), g.iter().map(|v| -v).collect(), nz)
                .or_else(|| least_squares_step(&jac, &g, nz))
            else {
                break;
            };
            let mut step = 1.0;
            let mut accepted = false;
            for _ in 0..12 {
                let trial: Vec<f64> = z.iter().zip(&dz).map(|(a, d)| a + step * d).collect();
                let gt = residual(&trial, &mut x);
                if norm(&gt) < g0 {
                    z = trial;
                    g = gt;
                    accepted = true;
                    break;
                }
                step *= 0.5;
            }
            if !accepted {
                break;
            }
        }
        residual(&z, &mut x);
        (x, if r.is_some() { z[0] } else { 0.0 })
    }
}

/// The step minimizing `|J dz + g|² + λ|dz|²` (λ small relative to `J`).
pub(crate) fn least_squares_step(jac: &[f64], g: &[f64], n: usize) -> Option<Vec<f64>> {
    let scale = jac.iter().fold(0.0_f64, |a, v| a.max(v.abs())).max(1e-300);
    let lambda = 1e-10 * scale * scale;
    let mut a = vec![0.0; n * n];
    let mut b = vec![0.0; n];
    for i in 0..n {
        for j in 0..n {
            a[i * n + j] = (0..n).map(|k| jac[k * n + i] * jac[k * n + j]).sum::<f64>();
        }
        a[i * n + i] += lambda;
        b[i] = -(0..n).map(|k| jac[k * n + i] * g[k]).sum::<f64>();
    }
    solve_small(a, b, n)
}

/// Solve the dense `n × n` system `a x = b` (row-major) by Gaussian
/// elimination with partial pivoting; `None` when singular or not finite.
fn solve_small(mut a: Vec<f64>, mut b: Vec<f64>, n: usize) -> Option<Vec<f64>> {
    for k in 0..n {
        let p = (k..n).max_by(|&i, &j| a[i * n + k].abs().total_cmp(&a[j * n + k].abs()))?;
        if !(a[p * n + k].abs() > 1e-300) {
            return None;
        }
        if p != k {
            for j in 0..n {
                a.swap(k * n + j, p * n + j);
            }
            b.swap(k, p);
        }
        for i in k + 1..n {
            let l = a[i * n + k] / a[k * n + k];
            for j in k..n {
                a[i * n + j] -= l * a[k * n + j];
            }
            b[i] -= l * b[k];
        }
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let s: f64 = (i + 1..n).map(|j| a[i * n + j] * x[j]).sum();
        x[i] = (b[i] - s) / a[i * n + i];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

// ---------------------------------------------------------------------------
// Templates: the compiled PSS/E models as graphs.

fn node(id: &str, block: Block, inputs: &[&str]) -> GraphNode {
    GraphNode {
        id: id.into(),
        block,
        inputs: inputs.iter().map(|s| (*s).to_string()).collect(),
        states: Vec::new(),
        start: None,
        pos: None,
    }
}

fn starting(mut n: GraphNode, a: f64, b: f64) -> GraphNode {
    n.start = Some([a, b]);
    n
}

fn named(mut n: GraphNode, states: &[&str]) -> GraphNode {
    n.states = states.iter().map(|s| (*s).to_string()).collect();
    n
}

fn input(id: &str, port: &str) -> GraphNode {
    node(id, Block::Input { port: port.into() }, &[])
}

fn inv(r: f64) -> f64 {
    if r < f64::EPSILON {
        0.0
    } else {
        1.0 / r
    }
}

/// The compiled model of a DYR record as a block graph (same blocks, same
/// state names) for every controller the engine compiles: SEXS, ESAC1A,
/// EXST1, TGOV1, GAST, HYGOV, IEEEST (input codes 1 and 3). `None`
/// otherwise.
#[must_use]
pub fn template(model: &powerio::DynamicModel) -> Option<BlockGraph> {
    use powerio::DynamicModel as M;
    let g = |slot, name: &str, nodes: Vec<GraphNode>, output: &str| BlockGraph {
        slot,
        name: name.into(),
        nodes,
        output: output.into(),
    };
    Some(match model {
        M::Sexs(m) => g(
            Slot::Exciter,
            "SEXS",
            vec![
                input("vt", "vt"),
                input("vs", "vs"),
                input("v_ref", "v_ref"),
                node(
                    "error",
                    Block::Sum {
                        signs: vec![1.0, 1.0, -1.0],
                    },
                    &["v_ref", "vs", "vt"],
                ),
                node(
                    "Vf",
                    Block::LagLimited {
                        k: m.k,
                        t: m.te,
                        min: m.emin,
                        max: m.emax,
                        clamp_output: false,
                    },
                    &["Vr"],
                ),
                node(
                    "Vr",
                    Block::LeadLag {
                        k: 1.0,
                        t1: m.tb * m.ta_tb,
                        t2: m.tb,
                    },
                    &["error"],
                ),
            ],
            "Vf",
        ),
        M::Exst1(m) => g(
            Slot::Exciter,
            "EXST1",
            vec![
                input("vt", "vt"),
                input("vs", "vs"),
                input("ifd", "xad_ifd"),
                input("v_ref", "v_ref"),
                node("Vm", Block::Lag { k: 1.0, t: m.tr }, &["vt"]),
                node(
                    "error",
                    Block::Sum {
                        signs: vec![1.0, 1.0, -1.0, -1.0],
                    },
                    &["v_ref", "vs", "Vm", "Vfb"],
                ),
                node(
                    "Vi",
                    Block::Clamp {
                        min: m.vimin,
                        max: m.vimax,
                    },
                    &["error"],
                ),
                node(
                    "Vrll",
                    Block::LeadLag {
                        k: 1.0,
                        t1: m.tc,
                        t2: m.tb,
                    },
                    &["Vi"],
                ),
                node("Vr", Block::Lag { k: m.ka, t: m.ta }, &["Vrll"]),
                node("Vfb", Block::Washout { k: m.kf, t: m.tf }, &["Vr"]),
                node("vt_min", Block::Gain { k: m.vrmin }, &["vt"]),
                node("vt_max", Block::Gain { k: m.vrmax }, &["vt"]),
                node("kc_ifd", Block::Gain { k: m.kc }, &["ifd"]),
                node(
                    "lo",
                    Block::Sum {
                        signs: vec![1.0, -1.0],
                    },
                    &["vt_min", "kc_ifd"],
                ),
                node(
                    "hi",
                    Block::Sum {
                        signs: vec![1.0, -1.0],
                    },
                    &["vt_max", "kc_ifd"],
                ),
                node("Efd", Block::ClampVar, &["Vr", "lo", "hi"]),
            ],
            "Efd",
        ),
        M::Esac1a(m) => {
            // As the compiled model: a zero TR runs the transducer at 1 s.
            let tr = if m.tr < f64::EPSILON { 1.0 } else { m.tr };
            let (sat_a, sat_b) = quadratic_saturation((m.e1, m.e2), (m.se_e1, m.se_e2)).ok()?;
            g(
                Slot::Exciter,
                "ESAC1A",
                vec![
                    input("vt", "vt"),
                    input("vs", "vs"),
                    input("ifd", "xad_ifd"),
                    input("v_ref", "v_ref"),
                    node("Vm", Block::Lag { k: 1.0, t: tr }, &["vt"]),
                    node(
                        "error",
                        Block::Sum {
                            signs: vec![1.0, 1.0, -1.0, -1.0],
                        },
                        &["v_ref", "vs", "Vm", "Vr3"],
                    ),
                    node(
                        "Vr1",
                        Block::LeadLag {
                            k: 1.0,
                            t1: m.tc,
                            t2: m.tb,
                        },
                        &["error"],
                    ),
                    node(
                        "Vr2",
                        Block::LagLimited {
                            k: m.ka,
                            t: m.ta,
                            min: m.vamin,
                            max: m.vamax,
                            clamp_output: false,
                        },
                        &["Vr1"],
                    ),
                    node(
                        "Vr",
                        Block::Clamp {
                            min: m.vrmin,
                            max: m.vrmax,
                        },
                        &["Vr2"],
                    ),
                    node(
                        "ve_in",
                        Block::Sum {
                            signs: vec![1.0, -1.0],
                        },
                        &["Vr", "VFE"],
                    ),
                    node("Ve", Block::Integrator { k: 1.0 / m.te }, &["ve_in"]),
                    node("Vr3", Block::Washout { k: m.kf, t: m.tf }, &["VFE"]),
                    node("kd_ifd", Block::Gain { k: m.kd }, &["ifd"]),
                    node("ke_ve", Block::Gain { k: m.ke }, &["Ve"]),
                    node("Se", Block::SatQuadratic { a: sat_a, b: sat_b }, &["Ve"]),
                    node("se_ve", Block::Product, &["Se", "Ve"]),
                    node(
                        "VFE",
                        Block::Sum {
                            signs: vec![1.0, 1.0, 1.0],
                        },
                        &["kd_ifd", "ke_ve", "se_ve"],
                    ),
                    node("kc_ifd", Block::Gain { k: m.kc }, &["ifd"]),
                    node("IN", Block::Divide, &["kc_ifd", "Ve"]),
                    node("FEX", Block::Rectifier, &["IN"]),
                    node("Efd", Block::Product, &["Ve", "FEX"]),
                ],
                "Efd",
            )
        }
        M::Gast(m) => g(
            Slot::Governor,
            "GAST",
            vec![
                input("omega", "omega"),
                input("p_ref", "p_ref"),
                node("one", Block::Const { value: 1.0 }, &[]),
                node(
                    "dw",
                    Block::Sum {
                        signs: vec![1.0, -1.0],
                    },
                    &["omega", "one"],
                ),
                node("droop", Block::Gain { k: inv(m.r) }, &["dw"]),
                node(
                    "reference",
                    Block::Sum {
                        signs: vec![1.0, -1.0],
                    },
                    &["p_ref", "droop"],
                ),
                named(
                    node(
                        "valve",
                        Block::LagLimited {
                            k: 1.0,
                            t: m.t1,
                            min: m.vmin,
                            max: m.vmax,
                            clamp_output: true,
                        },
                        &["fuel"],
                    ),
                    &["x_g1"],
                ),
                named(
                    node("turbine", Block::Lag { k: 1.0, t: m.t2 }, &["valve"]),
                    &["x_g2"],
                ),
                named(
                    node("exhaust", Block::Lag { k: 1.0, t: m.t3 }, &["turbine"]),
                    &["x_g3"],
                ),
                node("at", Block::Const { value: m.at }, &[]),
                node(
                    "margin",
                    Block::Sum {
                        signs: vec![1.0, -1.0],
                    },
                    &["at", "exhaust"],
                ),
                node("kt_margin", Block::Gain { k: m.kt }, &["margin"]),
                node(
                    "limit",
                    Block::Sum {
                        signs: vec![1.0, 1.0],
                    },
                    &["at", "kt_margin"],
                ),
                node("fuel", Block::Min { tie: 1e-4 }, &["reference", "limit"]),
                node("damping", Block::Gain { k: m.dturb }, &["dw"]),
                node(
                    "power",
                    Block::Sum {
                        signs: vec![1.0, -1.0],
                    },
                    &["turbine", "damping"],
                ),
                node("torque", Block::Divide, &["power", "omega"]),
            ],
            "torque",
        ),
        M::Hygov(m) => g(
            Slot::Governor,
            "HYGOV",
            vec![
                input("omega", "omega"),
                input("p_ref", "p_ref"),
                node("one", Block::Const { value: 1.0 }, &[]),
                node(
                    "dw",
                    Block::Sum {
                        signs: vec![1.0, -1.0],
                    },
                    &["omega", "one"],
                ),
                node("droop", Block::Gain { k: m.r }, &["gate"]),
                node(
                    "p_in",
                    Block::Sum {
                        signs: vec![1.0, -1.0, -1.0],
                    },
                    &["p_ref", "dw", "droop"],
                ),
                named(
                    node("filter", Block::Lag { k: 1.0, t: m.tf }, &["p_in"]),
                    &["x_g1"],
                ),
                named(
                    starting(
                        node(
                            "gate",
                            Block::PiLimited {
                                kp: 1.0 / m.r_temp,
                                ki: 1.0 / (m.r_temp * m.tr),
                                min: m.gmin,
                                max: m.gmax,
                                rate: Some(m.velm),
                            },
                            &["filter"],
                        ),
                        // At rest the gate equals the flow q = τm/At + qNL.
                        m.r_temp * m.tr / m.at,
                        m.r_temp * m.tr * m.qnl,
                    ),
                    &["x_g2"],
                ),
                named(
                    node("servo", Block::Lag { k: 1.0, t: m.tg }, &["gate"]),
                    &["x_g3"],
                ),
                node("q_over_g", Block::Divide, &["flow", "servo"]),
                node("head", Block::Product, &["q_over_g", "q_over_g"]),
                node(
                    "dh",
                    Block::Sum {
                        signs: vec![1.0, -1.0],
                    },
                    &["one", "head"],
                ),
                named(
                    starting(
                        node("flow", Block::Integrator { k: 1.0 / m.tw }, &["dh"]),
                        1.0 / m.at,
                        m.qnl,
                    ),
                    &["x_g4"],
                ),
                node("qnl", Block::Const { value: m.qnl }, &[]),
                node(
                    "q_net",
                    Block::Sum {
                        signs: vec![1.0, -1.0],
                    },
                    &["flow", "qnl"],
                ),
                node("qh", Block::Product, &["q_net", "head"]),
                node("p_turb", Block::Gain { k: m.at }, &["qh"]),
                node("dw_g", Block::Product, &["dw", "servo"]),
                node("damping", Block::Gain { k: m.dturb }, &["dw_g"]),
                node(
                    "power",
                    Block::Sum {
                        signs: vec![1.0, -1.0],
                    },
                    &["p_turb", "damping"],
                ),
                node("torque", Block::Divide, &["power", "omega"]),
            ],
            "torque",
        ),
        M::Tgov1(m) => g(
            Slot::Governor,
            "TGOV1",
            vec![
                input("omega", "omega"),
                input("p_ref", "p_ref"),
                node("one", Block::Const { value: 1.0 }, &[]),
                node(
                    "dw",
                    Block::Sum {
                        signs: vec![1.0, -1.0],
                    },
                    &["omega", "one"],
                ),
                node(
                    "error",
                    Block::Sum {
                        signs: vec![1.0, -1.0],
                    },
                    &["p_ref", "dw"],
                ),
                node("droop", Block::Gain { k: inv(m.r) }, &["error"]),
                named(
                    node(
                        "valve",
                        Block::LagLimited {
                            k: 1.0,
                            t: m.t1,
                            min: m.vmin,
                            max: m.vmax,
                            clamp_output: true,
                        },
                        &["droop"],
                    ),
                    &["x_g1"],
                ),
                named(
                    node(
                        "turbine",
                        Block::LeadLag {
                            k: 1.0,
                            t1: m.t2,
                            t2: m.t3,
                        },
                        &["valve"],
                    ),
                    &["x_g2"],
                ),
                node("damping", Block::Gain { k: m.dt }, &["dw"]),
                node(
                    "power",
                    Block::Sum {
                        signs: vec![1.0, -1.0],
                    },
                    &["turbine", "damping"],
                ),
                node("torque", Block::Divide, &["power", "omega"]),
            ],
            "torque",
        ),
        M::Ieeest(m) if m.input_code == 1 || m.input_code == 3 => {
            let source = if m.input_code == 3 {
                // PSS/E input code 3: the electrical power on `MBASE`, as
                // the compiled model reads it.
                vec![input("signal", "pe_m")]
            } else {
                vec![
                    input("omega", "omega"),
                    node("one", Block::Const { value: 1.0 }, &[]),
                    node(
                        "signal",
                        Block::Sum {
                            signs: vec![1.0, -1.0],
                        },
                        &["omega", "one"],
                    ),
                ]
            };
            let mut nodes = source;
            nodes.extend([
                input("vt", "vt"),
                named(
                    node(
                        "filter1",
                        Block::Lag2 {
                            k: 1.0,
                            t1: m.a3,
                            t2: m.a4,
                        },
                        &["signal"],
                    ),
                    &["x_p1", "x_p2"],
                ),
                named(
                    node(
                        "filter2",
                        Block::LeadLag2 {
                            t1: m.a1,
                            t2: m.a2,
                            t3: m.a5,
                            t4: m.a6,
                        },
                        &["filter1"],
                    ),
                    &["x_p3", "x_p4"],
                ),
                named(
                    node(
                        "ll1",
                        Block::LeadLag {
                            k: 1.0,
                            t1: m.t1,
                            t2: m.t2,
                        },
                        &["filter2"],
                    ),
                    &["x_p5"],
                ),
                named(
                    node(
                        "ll2",
                        Block::LeadLag {
                            k: 1.0,
                            t1: m.t3,
                            t2: m.t4,
                        },
                        &["ll1"],
                    ),
                    &["x_p6"],
                ),
                named(
                    node(
                        "washout",
                        Block::Washout {
                            k: m.ks * m.t5,
                            t: m.t6,
                        },
                        &["ll2"],
                    ),
                    &["x_p7"],
                ),
                node(
                    "limit",
                    Block::Clamp {
                        min: m.lsmin,
                        max: m.lsmax,
                    },
                    &["washout"],
                ),
                node(
                    "Vs",
                    Block::Gate {
                        lo: m.vcl,
                        hi: m.vcu,
                    },
                    &["limit", "vt"],
                ),
            ]);
            g(Slot::Stabilizer, "IEEEST", nodes, "Vs")
        }
        _ => return None,
    })
}

/// A graph for one controller slot of the generator `(bus, machine_id)`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelOverride {
    pub bus: usize,
    pub machine_id: String,
    pub graph: BlockGraph,
}

/// DYR text, optionally with controller graphs: either plain DYR, or a JSON
/// document `{"dyr": "...", "models": [ModelOverride, ...]}` (the form the
/// browser sends once a graph replaces a compiled controller).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DynamicInput {
    pub dyr: String,
    #[serde(default)]
    pub models: Vec<ModelOverride>,
}

impl DynamicInput {
    /// Read plain DYR text or the JSON form.
    ///
    /// # Errors
    /// When the JSON form does not parse.
    pub fn from_text(text: &str) -> Result<Self, String> {
        if text.trim_start().starts_with('{') {
            serde_json::from_str(text).map_err(|e| format!("dynamic data JSON: {e}"))
        } else {
            Ok(Self {
                dyr: text.to_string(),
                models: Vec::new(),
            })
        }
    }

    /// The graph for a slot of a generator, if one is given.
    #[must_use]
    pub fn graph_for(&self, bus: usize, machine_id: &str, slot: Slot) -> Option<&BlockGraph> {
        self.models
            .iter()
            .find(|m| m.bus == bus && m.machine_id == machine_id && m.graph.slot == slot)
            .map(|m| &m.graph)
    }
}

/// The template graph of the DYR record on `line`, as JSON.
///
/// # Errors
/// When the DYR text does not parse, no record starts on that line, or its
/// model has no template.
pub fn template_json(dyr_text: &str, line: usize) -> Result<String, String> {
    let input = DynamicInput::from_text(dyr_text)?;
    let dyr = powerio::parse_dyr(&input.dyr).map_err(|e| format!("DYR: {e}"))?;
    let rec = dyr
        .records()
        .iter()
        .find(|r| r.line() == line)
        .ok_or_else(|| format!("no record starts on line {line}"))?;
    let model = powerio::typed_model(rec).map_err(|e| e.to_string())?;
    let graph =
        template(&model).ok_or_else(|| format!("{} has no block template yet", model.name()))?;
    serde_json::to_string(&graph).map_err(|e| e.to_string())
}

/// Check a graph JSON (`BlockGraph`): `{"ok": true, "states": [...]}` or the
/// error, for the editor.
#[must_use]
pub fn check_graph_json(graph_json: &str) -> String {
    let r = serde_json::from_str::<BlockGraph>(graph_json)
        .map_err(|e| e.to_string())
        .and_then(|g| CompiledGraph::new(&g));
    match r {
        Ok(c) => serde_json::json!({ "ok": true, "states": c.state_names() }).to_string(),
        Err(e) => serde_json::json!({ "ok": false, "error": e }).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A graph with every block, each in every form its evaluation has
    /// (a tie, a bypassed gate, a zero time constant, a rate limit, …).
    fn every_block() -> BlockGraph {
        let lag_limited = |clamp_output| Block::LagLimited {
            k: 1.5,
            t: 0.3,
            min: -0.6,
            max: 0.8,
            clamp_output,
        };
        let pi = |rate| Block::PiLimited {
            kp: 2.0,
            ki: 5.0,
            min: -1.0,
            max: 1.2,
            rate,
        };
        BlockGraph {
            slot: Slot::Exciter,
            name: "every block".into(),
            nodes: vec![
                input("vt", "vt"),
                input("vs", "vs"),
                input("ifd", "xad_ifd"),
                node("c", Block::Const { value: 0.7 }, &[]),
                node("gain", Block::Gain { k: -2.5 }, &["vt"]),
                node(
                    "sum",
                    Block::Sum {
                        signs: vec![1.0, -1.0, 0.5, -0.25],
                    },
                    &["gain", "vs", "c", "lag"],
                ),
                node("product", Block::Product, &["vt", "vs", "ifd"]),
                node("divide", Block::Divide, &["sum", "c"]),
                node(
                    "clamp",
                    Block::Clamp {
                        min: -1.0,
                        max: 1.0,
                    },
                    &["divide"],
                ),
                node("clamp_var", Block::ClampVar, &["product", "vs", "vt"]),
                node("min_tie", Block::Min { tie: 0.1 }, &["gain", "vs"]),
                node("min_two", Block::Min { tie: 0.0 }, &["gain", "vs"]),
                node("min_three", Block::Min { tie: 0.1 }, &["gain", "vs", "ifd"]),
                node("max", Block::Max, &["gain", "vs", "c"]),
                node("rectifier", Block::Rectifier, &["ifd"]),
                node("sat", Block::SatQuadratic { a: 0.8, b: 0.3 }, &["vt"]),
                node("sat_none", Block::SatQuadratic { a: 0.8, b: 0.0 }, &["vt"]),
                node("gate", Block::Gate { lo: -0.5, hi: 0.5 }, &["gain", "vs"]),
                node(
                    "gate_open",
                    Block::Gate { lo: 0.0, hi: 0.5 },
                    &["gain", "vs"],
                ),
                node("lag", Block::Lag { k: 2.0, t: 0.5 }, &["clamp"]),
                node("lag_limited", lag_limited(true), &["sum"]),
                node("lag_state", lag_limited(false), &["lag"]),
                node(
                    "lead_lag",
                    Block::LeadLag {
                        k: 1.5,
                        t1: 0.2,
                        t2: 0.6,
                    },
                    &["lag_limited"],
                ),
                node(
                    "lead_lag_static",
                    Block::LeadLag {
                        k: 1.5,
                        t1: 0.2,
                        t2: 0.0,
                    },
                    &["gain"],
                ),
                node("washout", Block::Washout { k: 2.0, t: 2.0 }, &["lead_lag"]),
                node(
                    "washout_static",
                    Block::Washout { k: 2.0, t: 0.0 },
                    &["min_tie"],
                ),
                node("integrator", Block::Integrator { k: 3.0 }, &["washout"]),
                node("pi", pi(None), &["gate"]),
                node("pi_rate", pi(Some(0.2)), &["min_three"]),
                node(
                    "lag2",
                    Block::Lag2 {
                        k: 1.2,
                        t1: 0.4,
                        t2: 0.05,
                    },
                    &["pi"],
                ),
                node(
                    "lead_lag2",
                    Block::LeadLag2 {
                        t1: 0.3,
                        t2: 0.04,
                        t3: 0.2,
                        t4: 0.01,
                    },
                    &["lag2"],
                ),
                node(
                    "lead_lag2_static",
                    Block::LeadLag2 {
                        t1: 0.3,
                        t2: 0.0,
                        t3: 0.2,
                        t4: 0.01,
                    },
                    &["integrator"],
                ),
                node(
                    "out",
                    Block::Sum {
                        signs: vec![1.0; 16],
                    },
                    &[
                        "clamp_var",
                        "min_two",
                        "max",
                        "rectifier",
                        "sat",
                        "sat_none",
                        "gate_open",
                        "lag_state",
                        "lead_lag_static",
                        "washout_static",
                        "pi_rate",
                        "lead_lag2",
                        "lead_lag2_static",
                        "lag_limited",
                        "integrator",
                        "divide",
                    ],
                ),
            ],
            output: "out".into(),
        }
    }

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/case_data.rs"
    ));

    /// The graphs of the example cases: the template of every DYR record
    /// that has one, and the cases' own models.
    fn example_graphs() -> Vec<BlockGraph> {
        let mut graphs = Vec::new();
        // The shipped case, and the cases kept outside the repository where they are present.
        let mut folders = rms_example_roots();
        if let Some(data) = std::env::var_os("TELLEGEN_CASE_DATA") {
            folders.push(std::path::PathBuf::from(data).join("rms_validation"));
        }
        for examples in folders {
            let Ok(dirs) = std::fs::read_dir(examples) else {
                continue;
            };
            for dir in dirs.filter_map(Result::ok).map(|d| d.path()) {
                if let Ok(text) = std::fs::read_to_string(dir.join("case.dyr")) {
                    if let Ok(dyr) = powerio::parse_dyr(&text) {
                        graphs.extend(
                            dyr.records()
                                .iter()
                                .filter_map(|r| powerio::typed_model(r).ok())
                                .filter_map(|m| template(&m)),
                        );
                    }
                }
            }
        }
        // The model library's records: every model, also those WECC 240 does
        // not use.
        for m in super::super::library::LIBRARY {
            if let Ok(dyr) = powerio::parse_dyr(&super::super::library::record(m, 1, "1")) {
                graphs.extend(
                    dyr.records()
                        .iter()
                        .filter_map(|r| powerio::typed_model(r).ok())
                        .filter_map(|m| template(&m)),
                );
            }
        }
        graphs
    }

    /// The operation list computes what the blocks define, to the bit: every
    /// block in every form, and every graph of the example cases, at
    /// states and inputs inside, at and beyond the limits.
    #[test]
    fn operation_list_is_the_blocks_to_the_bit() {
        let mut graphs = vec![every_block()];
        // A dozen parameter sets of each model.
        let mut taken = HashMap::new();
        for g in example_graphs() {
            let count = taken.entry(g.name.clone()).or_insert(0);
            if *count < 12 && !graphs.contains(&g) {
                *count += 1;
                graphs.push(g);
            }
        }
        let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let u = (seed >> 11) as f64 / (1u64 << 53) as f64;
            match (seed >> 5) % 16 {
                0 => 0.0,
                1 => (u - 0.5) * 100.0,
                // The limits and thresholds of the blocks above and of the
                // templates are often among these.
                2 => [-1.0, 1.0, -0.6, 0.8, 0.5, -0.5, 0.433, 0.75, 1.2][(seed >> 9) as usize % 9],
                _ => (u - 0.5) * 4.0,
            }
        };
        let (mut kinds, mut evaluated) = (std::collections::HashSet::new(), 0);
        for graph in &graphs {
            let g = CompiledGraph::new(graph).unwrap();
            for op in &g.tape {
                kinds.insert(std::mem::discriminant(op));
            }
            let (n, k) = (g.blocks.len(), g.n_states());
            let ports = graph.slot.ports().len();
            for trial in 0..300 {
                let x: Vec<f64> = (0..k).map(|_| next()).collect();
                // Also with a port missing: it reads as zero.
                let p: Vec<f64> = (0..ports - usize::from(trial % 50 == 49))
                    .map(|_| next())
                    .collect();
                let (mut f, mut y) = (vec![f64::NAN; k], vec![0.0; n]);
                let (mut f_ref, mut y_ref) = (vec![f64::NAN; k], vec![0.0; n]);
                g.run(&x, &p, &mut f, &mut y);
                g.run_blocks(&x, &p, &mut f_ref, &mut y_ref);
                let bits = |v: &[f64]| v.iter().map(|a| a.to_bits()).collect::<Vec<_>>();
                assert_eq!(bits(&y), bits(&y_ref), "{}: outputs", graph.name);
                assert_eq!(bits(&f), bits(&f_ref), "{}: derivatives", graph.name);
                // The slot's output and the signals are those of the list.
                let mut f_eval = vec![0.0; k];
                assert_eq!(g.eval(&x, &p, &mut f_eval).to_bits(), y[g.output].to_bits());
                assert_eq!(bits(&g.signals(&x, &p)), bits(&y));
                evaluated += 1;
            }
        }
        // Every kind of operation was run.
        assert_eq!(kinds.len(), 24, "{} graphs", graphs.len());
        assert!(
            graphs.len() > 15 && evaluated > 4500,
            "{} graphs",
            graphs.len()
        );
    }

    #[test]
    fn rejects_algebraic_loops_and_bad_wiring() {
        let g = |nodes: Vec<GraphNode>| BlockGraph {
            slot: Slot::Exciter,
            name: String::new(),
            nodes,
            output: "a".into(),
        };
        let looped = g(vec![
            node("a", Block::Gain { k: 1.0 }, &["b"]),
            node("b", Block::Gain { k: 1.0 }, &["a"]),
        ]);
        assert!(CompiledGraph::new(&looped)
            .unwrap_err()
            .contains("algebraic loop"));
        let lag_loop = g(vec![
            node("a", Block::Lag { k: 1.0, t: 0.1 }, &["b"]),
            node("b", Block::Gain { k: -1.0 }, &["a"]),
        ]);
        assert!(CompiledGraph::new(&lag_loop).is_ok());
        let bad_port = g(vec![input("a", "omega")]);
        assert!(CompiledGraph::new(&bad_port)
            .unwrap_err()
            .contains("no port"));
        let arity = g(vec![node("a", Block::Divide, &["a"])]);
        assert!(CompiledGraph::new(&arity).unwrap_err().contains("takes 2"));
    }

    /// The signals of every node, the output among them.
    #[test]
    fn signals_name_every_node() {
        let g = BlockGraph {
            slot: Slot::Governor,
            name: String::new(),
            nodes: vec![
                input("w", "omega"),
                node("k", Block::Gain { k: 2.0 }, &["w"]),
            ],
            output: "k".into(),
        };
        let c = CompiledGraph::new(&g).unwrap();
        assert_eq!(c.node_ids().collect::<Vec<_>>(), ["w", "k"]);
        assert_eq!(c.signals(&[], &[1.5, 0.0]), [1.5, 3.0]);
    }

    /// A first-order loop at rest and its step response against the
    /// closed form (`K/(1+sT)` from 0 to K u).
    #[test]
    fn lag_derivative_and_steady_state() {
        let graph = BlockGraph {
            slot: Slot::Exciter,
            name: String::new(),
            nodes: vec![
                input("u", "v_ref"),
                node("y", Block::Lag { k: 2.0, t: 0.5 }, &["u"]),
            ],
            output: "y".into(),
        };
        let c = CompiledGraph::new(&graph).unwrap();
        let mut m = [0.0];
        c.mass(&mut m);
        assert_eq!(m, [0.5]);
        let mut f = [0.0];
        let y = c.eval(&[0.3], &[1.0, 0.0, 0.0, 0.7], &mut f);
        assert_eq!(y, 0.3);
        assert!((f[0] - (2.0 * 0.7 - 0.3)).abs() < 1e-15);
        let (x, r) = c.initial_guess(&[1.0, 0.0, 0.0, 1.0], 1.6);
        assert!((r - 0.8).abs() < 1e-10 && (x[0] - 1.6).abs() < 1e-10);
    }
}
