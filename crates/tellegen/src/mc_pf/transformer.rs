//! Finite-leakage OpenDSS transformer primitive for the multiconductor solver.
//!
//! The implementation follows the Version 8 OpenDSS `CalcY_Terminal` path:
//! winding leakage is assembled as `ZB`, inverted on the one-volt winding
//! base, transformed by the `[-1 I]` winding-difference map, then referred to
//! the physical coil terminals.  The final terminal matrix is assembled with
//! the WYE/DELTA incidence map, so a delta winding is represented by its
//! actual line-to-line coils rather than by independent phase ratios.
//!
//! This module deliberately prepares one finite-leakage two-winding element.
//! Ideal transformers need voltage constraints and are rejected here; silently
//! returning a zero admittance would disconnect the network.  Multiwinding
//! data is also rejected until the corresponding terminal and reader contract
//! is reviewed.

use num_complex::{Complex64, ComplexFloat};
use powerio::dist::{DistTransformer, DistWinding, DistWindingConn};
use serde_json::Value;
use std::fmt;

pub type ComplexMatrix = Vec<Vec<Complex64>>;

/// A physical terminal used by a prepared primitive.  Repeated terminal
/// names (for example a common neutral) occur once in `TransformerPrimitive`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransformerTerminal {
    pub bus: String,
    pub terminal: String,
}

/// Why preparation was refused.  Errors are intentionally descriptive: a
/// transformer that cannot be represented must fail preflight rather than
/// becoming a zero or guessed branch.
#[derive(Clone, Debug, PartialEq)]
pub enum TransformerError {
    Unsupported {
        transformer: String,
        reason: String,
    },
    Invalid {
        transformer: String,
        field: String,
        reason: String,
    },
    SingularLeakage {
        transformer: String,
    },
}

impl fmt::Display for TransformerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported {
                transformer,
                reason,
            } => {
                write!(f, "transformer `{transformer}` is unsupported: {reason}")
            }
            Self::Invalid {
                transformer,
                field,
                reason,
            } => write!(
                f,
                "transformer `{transformer}` has invalid {field}: {reason}"
            ),
            Self::SingularLeakage { transformer } => {
                write!(
                    f,
                    "transformer `{transformer}` has singular leakage impedance"
                )
            }
        }
    }
}

impl std::error::Error for TransformerError {}

/// A prepared terminal-level transformer primitive.
#[derive(Clone, Debug)]
pub struct TransformerPrimitive {
    pub name: String,
    pub terminals: Vec<TransformerTerminal>,
    /// Leakage and winding neutral grounding contributions.
    pub y_series: ComplexMatrix,
    /// Magnetizing/core-loss contributions.
    pub y_shunt: ComplexMatrix,
    /// `y_series + y_shunt`, in `terminals` order.
    pub y_prim: ComplexMatrix,
    /// WYE neutral terminals that are explicitly solid grounded. These are
    /// exact Dirichlet constraints applied by network preparation.
    pub grounded_terminals: Vec<TransformerTerminal>,
    /// Source-compatible delta phase orientation (`+1` or `-1`).
    pub delta_direction: i8,
    /// Winding-domain one-volt leakage matrix after inversion and the
    /// winding-difference transform. This is useful for evidence and tests.
    pub y_1volt: ComplexMatrix,
}

/// Compatibility alias for callers that use the preparation vocabulary.
pub type PreparedTransformer = TransformerPrimitive;

impl TransformerPrimitive {
    #[must_use]
    pub fn dimension(&self) -> usize {
        self.terminals.len()
    }

    /// Add this primitive into a global complex nodal matrix. `lookup` maps a
    /// `(bus, terminal)` pair to a global row/column; returning `None` means
    /// the caller has intentionally eliminated that terminal as ground.
    pub fn stamp_into<F>(
        &self,
        ybus: &mut [Vec<Complex64>],
        lookup: F,
    ) -> Result<(), TransformerError>
    where
        F: Fn(&str, &str) -> Option<usize>,
    {
        if ybus.iter().any(|row| row.len() != ybus.len()) {
            return Err(TransformerError::Invalid {
                transformer: self.name.clone(),
                field: "global matrix".into(),
                reason: "expected a square matrix".into(),
            });
        }
        for (i, a) in self.terminals.iter().enumerate() {
            let Some(ia) = lookup(&a.bus, &a.terminal) else {
                continue;
            };
            if ia >= ybus.len() {
                return Err(TransformerError::Invalid {
                    transformer: self.name.clone(),
                    field: "global terminal index".into(),
                    reason: format!(
                        "index {ia} is outside a {}x{} matrix",
                        ybus.len(),
                        ybus.len()
                    ),
                });
            }
            for (j, b) in self.terminals.iter().enumerate() {
                let Some(ib) = lookup(&b.bus, &b.terminal) else {
                    continue;
                };
                if ib >= ybus.len() {
                    return Err(TransformerError::Invalid {
                        transformer: self.name.clone(),
                        field: "global terminal index".into(),
                        reason: format!(
                            "index {ib} is outside a {}x{} matrix",
                            ybus.len(),
                            ybus.len()
                        ),
                    });
                }
                ybus[ia][ib] += self.y_prim[i][j];
            }
        }
        Ok(())
    }
}

/// Named constructor used by network assembly code.
pub fn build_transformer_yprim(
    transformer: &DistTransformer,
) -> Result<TransformerPrimitive, TransformerError> {
    prepare_transformer(transformer)
}

/// Prepare one finite-leakage, two-winding transformer from the canonical
/// PowerIO representation.
pub fn prepare_transformer(
    transformer: &DistTransformer,
) -> Result<TransformerPrimitive, TransformerError> {
    validate_shape(transformer)?;
    let w1 = &transformer.windings[0];
    let w2 = &transformer.windings[1];
    let phases = transformer.phases;
    let name = transformer.name.clone();

    let direction = delta_direction(transformer);
    let directions = [
        delta_direction_for(transformer, 0, direction),
        delta_direction_for(transformer, 1, direction),
    ];
    let vbase = [coil_voltage(w1, phases), coil_voltage(w2, phases)];
    let zbase = phases as f64 / w1.s_rating;

    // DistWinding::r_pct is percent of the winding's own base. OpenDSS's
    // ZB relation uses winding 1's common power base, so refer each physical
    // winding resistance to that base before constructing ZB.
    // `r_pct` is percent of each winding's own base.  The source's Rpu
    // values are expressed on winding 1's common power base, therefore the
    // second winding's value scales by S1/S2.  The voltage ratio belongs to
    // the later terminal transformation and must not be folded into Rpu.
    let r_common = [
        w1.r_pct / 100.0,
        w2.r_pct / 100.0 * w1.s_rating / w2.s_rating,
    ];
    let z12 = Complex64::new(
        (r_common[0] + r_common[1]) * zbase,
        transformer.xsc_pct[0] / 100.0 * zbase,
    );
    // This is a scalar two-winding leakage impedance.  Do not use an
    // absolute pivot cutoff here: a valid high-rating transformer can have a
    // one-volt-base impedance below 1e-14 ohm.  A zero or non-finite scalar
    // is the only singular case for this primitive.
    if !z12.re.is_finite() || !z12.im.is_finite() || z12.norm() == 0.0 {
        return Err(TransformerError::SingularLeakage {
            transformer: name.clone(),
        });
    }
    let yb = z12.recip();
    // A = [-1, 1], Y_1Volt = Aᵀ YB A.
    let y_1volt = vec![vec![yb, -yb], vec![-yb, yb]];

    let mut terminals = Vec::new();
    let mut y_series = Vec::new();
    let mut y_shunt = Vec::new();
    let mut grounded_terminals = Vec::new();
    // Build the unique terminal list once. Coils from all phases then stamp
    // through the same physical neutral column when a neutral is explicit.
    for winding in [w1, w2] {
        for terminal in winding
            .terminal_map
            .iter()
            .take(terminal_count(winding, phases))
        {
            if !terminals
                .iter()
                .any(|t: &TransformerTerminal| t.bus == winding.bus && t.terminal == *terminal)
            {
                terminals.push(TransformerTerminal {
                    bus: winding.bus.clone(),
                    terminal: terminal.clone(),
                });
            }
        }
    }
    let nterm = terminals.len();
    if nterm == 0 {
        return Err(TransformerError::Invalid {
            transformer: name,
            field: "terminal_map".into(),
            reason: "no physical terminals".into(),
        });
    }
    y_series.resize_with(nterm, || vec![Complex64::new(0.0, 0.0); nterm]);
    y_shunt.resize_with(nterm, || vec![Complex64::new(0.0, 0.0); nterm]);

    // The physical terminal matrix is the sum over independent phase
    // copies of Bᵀ Y_1Volt B, where B contains the turns/base factors.
    for phase in 0..phases {
        let mut b = vec![vec![Complex64::new(0.0, 0.0); nterm]; 2];
        let (a1, z1) = coil_refs(w1, phase, phases, directions[0], &terminals, &name)?;
        let (a2, z2) = coil_refs(w2, phase, phases, directions[1], &terminals, &name)?;
        let k1 = Complex64::new(1.0 / (vbase[0] * checked_tap(w1, &name)?), 0.0);
        let k2 = Complex64::new(1.0 / (vbase[1] * checked_tap(w2, &name)?), 0.0);
        if let Some(i) = a1 {
            b[0][i] += k1;
        }
        if let Some(i) = z1 {
            b[0][i] -= k1;
        }
        if let Some(i) = a2 {
            b[1][i] += k2;
        }
        if let Some(i) = z2 {
            b[1][i] -= k2;
        }
        for i in 0..nterm {
            for j in 0..nterm {
                let mut value = Complex64::new(0.0, 0.0);
                for p in 0..2 {
                    for q in 0..2 {
                        value += b[p][i] * y_1volt[p][q] * b[q][j];
                    }
                }
                y_series[i][j] += value;
            }
        }

        // Explicit neutral impedance is one shunt from the physical WYE
        // neutral conductor to ground, shared by all phase coils.
        if phase == 0 {
            for (idx, winding) in [w1, w2].iter().enumerate() {
                if winding.conn == DistWindingConn::Wye
                    && (winding.r_neutral.is_some() || winding.x_neutral.is_some())
                {
                    let neutral = neutral_ref(winding, phases, &terminals, &name)?;
                    let r = winding.r_neutral.unwrap_or(0.0);
                    let x = winding.x_neutral.unwrap_or(0.0);
                    if r < 0.0 || !r.is_finite() || !x.is_finite() {
                        if r < 0.0 && r.is_finite() && x.is_finite() {
                            // OpenDSS uses a negative RNeut as the explicit
                            // floating-neutral sentinel.
                            continue;
                        }
                        return Err(TransformerError::Invalid {
                            transformer: name.clone(),
                            field: format!("windings[{idx}].neutral impedance"),
                            reason: "expected finite nonnegative resistance and reactance".into(),
                        });
                    }
                    if r == 0.0 && x == 0.0 {
                        let terminal = winding.terminal_map.get(phases).ok_or_else(|| {
                            TransformerError::Invalid {
                                transformer: name.clone(),
                                field: format!("windings[{idx}].terminal_map"),
                                reason: "solid WYE neutral has no terminal".into(),
                            }
                        })?;
                        grounded_terminals.push(TransformerTerminal {
                            bus: winding.bus.clone(),
                            terminal: terminal.clone(),
                        });
                        continue;
                    } else {
                        let y = Complex64::new(1.0, 0.0) / Complex64::new(r, x);
                        if let Some(i) = neutral {
                            y_series[i][i] += y;
                        }
                    }
                } else if winding.conn == DistWindingConn::Delta
                    && (winding.r_neutral.is_some() || winding.x_neutral.is_some())
                {
                    return Err(TransformerError::Unsupported {
                        transformer: name.clone(),
                        reason:
                            "neutral impedance on a delta winding has no physical neutral terminal"
                                .into(),
                    });
                }
            }
        }
    }

    add_excitation(transformer, &mut y_shunt, &terminals)?;
    let mut y_prim = y_series.clone();
    for i in 0..nterm {
        for j in 0..nterm {
            y_prim[i][j] += y_shunt[i][j];
        }
    }
    Ok(TransformerPrimitive {
        name,
        terminals,
        y_series,
        y_shunt,
        y_prim,
        grounded_terminals,
        delta_direction: direction,
        y_1volt,
    })
}

fn validate_shape(t: &DistTransformer) -> Result<(), TransformerError> {
    let name = t.name.clone();
    if t.windings.len() != 2 {
        return Err(TransformerError::Unsupported {
            transformer: name,
            reason: format!(
                "{} windings are present; only finite two-winding primitives are supported",
                t.windings.len()
            ),
        });
    }
    if t.phases == 0 || t.phases > 3 {
        return Err(TransformerError::Unsupported {
            transformer: t.name.clone(),
            reason: format!(
                "{} phases are outside the supported 1, 2, and 3 phase shapes",
                t.phases
            ),
        });
    }
    if t.xsc_pct.len() != 1 {
        return Err(TransformerError::Invalid {
            transformer: t.name.clone(),
            field: "xsc_pct".into(),
            reason: "two-winding transformers require exactly one XHL value".into(),
        });
    }
    if !t.xsc_pct[0].is_finite() || t.xsc_pct[0] < 0.0 {
        return Err(TransformerError::Invalid {
            transformer: t.name.clone(),
            field: "xsc_pct[0]".into(),
            reason: "must be finite and nonnegative".into(),
        });
    }
    for (idx, w) in t.windings.iter().enumerate() {
        if !w.v_ref.is_finite() || w.v_ref <= 0.0 {
            return Err(TransformerError::Invalid {
                transformer: t.name.clone(),
                field: format!("windings[{idx}].v_ref"),
                reason: "must be positive and finite".into(),
            });
        }
        if !w.s_rating.is_finite() || w.s_rating <= 0.0 {
            return Err(TransformerError::Invalid {
                transformer: t.name.clone(),
                field: format!("windings[{idx}].s_rating"),
                reason: "must be positive and finite".into(),
            });
        }
        if !w.r_pct.is_finite() || w.r_pct < 0.0 {
            return Err(TransformerError::Invalid {
                transformer: t.name.clone(),
                field: format!("windings[{idx}].r_pct"),
                reason: "must be finite and nonnegative".into(),
            });
        }
        checked_tap(w, &t.name)?;
        let expected = terminal_count(w, t.phases);
        if t.phases == 1 && w.conn == DistWindingConn::Delta && w.terminal_map.len() < 2 {
            return Err(TransformerError::Invalid {
                transformer: t.name.clone(),
                field: format!("windings[{idx}].terminal_map"),
                reason: "a one-phase DELTA winding requires two coil endpoints".into(),
            });
        }
        if w.terminal_map.len() != expected
            && !(w.conn == DistWindingConn::Wye && w.terminal_map.len() == t.phases)
        {
            return Err(TransformerError::Invalid {
                transformer: t.name.clone(),
                field: format!("windings[{idx}].terminal_map"),
                reason: format!("expected {expected} terminals (or {} phase terminals with an implicit WYE ground), got {}", t.phases, w.terminal_map.len()),
            });
        }
    }
    let explicit_ideal = t.xsc_pct[0] == 0.0 && t.windings.iter().all(|w| w.r_pct == 0.0);
    if explicit_ideal {
        return Err(TransformerError::Unsupported {
            transformer: t.name.clone(),
            reason: "zero leakage requires ideal voltage constraints and cannot be represented by an admittance primitive".into(),
        });
    }
    Ok(())
}

fn checked_tap(w: &DistWinding, name: &str) -> Result<f64, TransformerError> {
    if !w.tap.is_finite() || w.tap <= 0.0 {
        return Err(TransformerError::Invalid {
            transformer: name.to_string(),
            field: "winding.tap".into(),
            reason: "must be positive and finite".into(),
        });
    }
    Ok(w.tap)
}

fn terminal_count(w: &DistWinding, phases: usize) -> usize {
    if phases == 1 {
        2.min(w.terminal_map.len())
    } else if w.conn == DistWindingConn::Wye {
        phases + 1
    } else {
        phases
    }
}

fn coil_voltage(w: &DistWinding, phases: usize) -> f64 {
    if w.conn == DistWindingConn::Wye && matches!(phases, 2 | 3) {
        w.v_ref / 3f64.sqrt()
    } else {
        w.v_ref
    }
}

fn terminal_index(terminals: &[TransformerTerminal], bus: &str, terminal: &str) -> Option<usize> {
    terminals
        .iter()
        .position(|t| t.bus == bus && t.terminal == terminal)
}

fn coil_refs(
    w: &DistWinding,
    phase: usize,
    phases: usize,
    direction: i8,
    terminals: &[TransformerTerminal],
    name: &str,
) -> Result<(Option<usize>, Option<usize>), TransformerError> {
    let first = w
        .terminal_map
        .get(phase)
        .ok_or_else(|| TransformerError::Invalid {
            transformer: name.to_string(),
            field: "terminal_map".into(),
            reason: format!("missing phase terminal {phase}"),
        })?;
    let second_name = if phases == 1 {
        w.terminal_map.get(1)
    } else if w.conn == DistWindingConn::Wye {
        w.terminal_map.get(phases)
    } else {
        let next = ((phase as i32 + direction as i32).rem_euclid(phases as i32)) as usize;
        w.terminal_map.get(next)
    };
    let first_idx = terminal_index(terminals, &w.bus, first);
    let second_idx = second_name.and_then(|s| terminal_index(terminals, &w.bus, s));
    Ok((first_idx, second_idx))
}

fn neutral_ref(
    w: &DistWinding,
    phases: usize,
    terminals: &[TransformerTerminal],
    name: &str,
) -> Result<Option<usize>, TransformerError> {
    if w.terminal_map.len() <= phases {
        return Err(TransformerError::Unsupported {
            transformer: name.to_string(),
            reason: "neutral impedance is declared but the WYE neutral is implicit ground".into(),
        });
    }
    Ok(terminal_index(terminals, &w.bus, &w.terminal_map[phases]))
}

fn value_number(extras: &std::collections::BTreeMap<String, Value>, key: &str) -> Option<f64> {
    extras.get(key).and_then(Value::as_f64)
}

fn add_excitation(
    t: &DistTransformer,
    y: &mut ComplexMatrix,
    terminals: &[TransformerTerminal],
) -> Result<(), TransformerError> {
    let name = &t.name;
    let explicit = t.extras.get("no_load_shunt");
    let legacy = value_number(&t.extras, "g_no_load")
        .or_else(|| value_number(&t.extras, "b_no_load"))
        .is_some();
    if explicit.is_some() && legacy {
        return Err(TransformerError::Invalid {
            transformer: name.clone(),
            field: "no_load_shunt".into(),
            reason: "cannot coexist with g_no_load or b_no_load".into(),
        });
    }
    if let Some(shunt) = explicit {
        let obj = shunt.as_object().ok_or_else(|| TransformerError::Invalid {
            transformer: name.clone(),
            field: "no_load_shunt".into(),
            reason: "expected an object containing winding, g, and b".into(),
        })?;
        let winding = obj.get("winding").and_then(Value::as_u64).ok_or_else(|| {
            TransformerError::Invalid {
                transformer: name.clone(),
                field: "no_load_shunt.winding".into(),
                reason: "expected one-based integer".into(),
            }
        })?;
        if winding == 0 || winding > 2 {
            return Err(TransformerError::Invalid {
                transformer: name.clone(),
                field: "no_load_shunt.winding".into(),
                reason: "must be 1 or 2".into(),
            });
        }
        let g = obj
            .get("g")
            .and_then(Value::as_f64)
            .ok_or_else(|| TransformerError::Invalid {
                transformer: name.clone(),
                field: "no_load_shunt.g".into(),
                reason: "expected finite nonnegative siemens".into(),
            })?;
        let b = obj
            .get("b")
            .and_then(Value::as_f64)
            .ok_or_else(|| TransformerError::Invalid {
                transformer: name.clone(),
                field: "no_load_shunt.b".into(),
                reason: "expected finite siemens".into(),
            })?;
        if !g.is_finite() || g < 0.0 || !b.is_finite() {
            return Err(TransformerError::Invalid {
                transformer: name.clone(),
                field: "no_load_shunt".into(),
                reason: "requires finite g >= 0 and finite b".into(),
            });
        }
        let w = &t.windings[winding as usize - 1];
        let direction = delta_direction_for(t, winding as usize - 1, delta_direction(t));
        for p in 0..t.phases {
            let (a, z) = coil_refs(w, p, t.phases, direction, terminals, name)?;
            add_coil_shunt(y, a, z, Complex64::new(g, b));
        }
        return Ok(());
    }
    if legacy {
        // The tagged BMOPF fields are from-winding per-coil siemens.
        let g = value_number(&t.extras, "g_no_load").unwrap_or(0.0);
        let b = value_number(&t.extras, "b_no_load").unwrap_or(0.0);
        if !g.is_finite() || g < 0.0 || !b.is_finite() {
            return Err(TransformerError::Invalid {
                transformer: name.clone(),
                field: "g_no_load/b_no_load".into(),
                reason: "requires finite g >= 0 and finite b".into(),
            });
        }
        let w = &t.windings[0];
        let direction = delta_direction_for(t, 0, delta_direction(t));
        for p in 0..t.phases {
            let (a, z) = coil_refs(w, p, t.phases, direction, terminals, name)?;
            add_coil_shunt(y, a, z, Complex64::new(g, b));
        }
        return Ok(());
    }
    // Legacy OpenDSS percentages are retained by the reader for some source
    // paths. Their reference implementation places the branch on winding 2.
    let loss = value_number(&t.extras, "%noloadloss");
    let imag = value_number(&t.extras, "%imag");
    if loss.is_none() && imag.is_none() {
        return Ok(());
    }
    let loss = loss.unwrap_or(0.0);
    let imag = imag.unwrap_or(0.0);
    if !loss.is_finite() || loss < 0.0 || !imag.is_finite() || imag < 0.0 {
        return Err(TransformerError::Invalid {
            transformer: name.clone(),
            field: "%noloadloss/%imag".into(),
            reason: "requires finite nonnegative percentages".into(),
        });
    }
    let w = &t.windings[1];
    let v = coil_voltage(w, t.phases) * w.tap;
    let ybase = t.windings[0].s_rating / (t.phases as f64 * v * v);
    let shunt = Complex64::new(loss / 100.0 * ybase, -imag / 100.0 * ybase);
    let direction = delta_direction_for(t, 1, delta_direction(t));
    for p in 0..t.phases {
        let (a, z) = coil_refs(w, p, t.phases, direction, terminals, name)?;
        add_coil_shunt(y, a, z, shunt);
    }
    Ok(())
}

fn add_coil_shunt(y: &mut ComplexMatrix, a: Option<usize>, z: Option<usize>, value: Complex64) {
    if let Some(i) = a {
        y[i][i] += value;
    }
    if let Some(i) = z {
        y[i][i] += value;
    }
    if let (Some(i), Some(j)) = (a, z) {
        y[i][j] -= value;
        y[j][i] -= value;
    }
}

fn delta_direction(t: &DistTransformer) -> i8 {
    let a = &t.windings[0];
    let b = &t.windings[1];
    // BMOPF's conventional three-phase subtypes carry the physical phase
    // orientation explicitly.  Preserve that producer contract when it is
    // present; a raw DSS DistTransformer falls through to OpenDSS's
    // high-voltage/HVLeadsLV rule below.
    if let Some(subtype) = t.extras.get("bmopf_subtype").and_then(Value::as_str) {
        match subtype {
            "wye_delta" => return 1,
            "delta_wye" => return -1,
            _ => {}
        }
    }
    if a.conn == b.conn {
        return 1;
    }
    let hv = if a.v_ref >= b.v_ref { a } else { b };
    let hv_leads_lv = t
        .extras
        .get("hv_leads_lv")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    match hv.conn {
        DistWindingConn::Wye => {
            if hv_leads_lv {
                -1
            } else {
                1
            }
        }
        DistWindingConn::Delta => {
            if hv_leads_lv {
                1
            } else {
                -1
            }
        }
        _ => 1,
    }
}

fn delta_direction_for(t: &DistTransformer, winding: usize, fallback: i8) -> i8 {
    if t.extras.get("bmopf_subtype").and_then(Value::as_str) != Some("n_winding") {
        return fallback;
    }
    t.extras
        .get("bmopf_delta_rolls")
        .and_then(Value::as_object)
        .and_then(|rolls| rolls.get(&(winding + 1).to_string()))
        .and_then(Value::as_i64)
        .filter(|roll| *roll == -1 || *roll == 1)
        // BMOPFTools' n-winding schema defaults DELTA rolls to +1;
        // OpenDSS-origin data carries its standard -1 explicitly.
        .map_or(1, |roll| roll as i8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use powerio::dist::{DistTransformer, DistWinding, DistWindingConn};

    fn transformer(conn1: DistWindingConn, conn2: DistWindingConn) -> DistTransformer {
        let mut w1 = DistWinding::new("hv", vec!["1".into()], conn1, 12_470.0, 300_000.0);
        w1.terminal_map = if conn1 == DistWindingConn::Wye {
            vec!["1".into(), "2".into(), "3".into(), "0".into()]
        } else {
            vec!["1".into(), "2".into(), "3".into()]
        };
        w1.r_pct = 0.5;
        let mut w2 = DistWinding::new("lv", vec!["1".into()], conn2, 480.0, 300_000.0);
        w2.terminal_map = if conn2 == DistWindingConn::Wye {
            vec!["1".into(), "2".into(), "3".into(), "0".into()]
        } else {
            vec!["1".into(), "2".into(), "3".into()]
        };
        w2.r_pct = 0.5;
        DistTransformer::new("t", vec![w1, w2], vec![6.0], 3)
    }

    fn dss_golden_transformer(conn1: DistWindingConn, conn2: DistWindingConn) -> DistTransformer {
        let mut t = transformer(conn1, conn2);
        for winding in &mut t.windings {
            winding.tap = if winding.bus == "hv" { 1.03 } else { 0.97 };
        }
        t
    }

    fn dss_golden_n_winding(roll1: i8, roll2: i8) -> DistTransformer {
        let mut t = dss_golden_transformer(DistWindingConn::Delta, DistWindingConn::Delta);
        t.extras
            .insert("bmopf_subtype".into(), Value::String("n_winding".into()));
        t.extras.insert(
            "bmopf_delta_rolls".into(),
            serde_json::json!({"1": roll1, "2": roll2}),
        );
        t
    }

    fn assert_entry(p: &TransformerPrimitive, row: usize, col: usize, re: f64, im: f64) {
        let got = p.y_prim[row][col];
        let expected = Complex64::new(re, im);
        assert!(
            (got - expected).norm() < 1.0e-11,
            "Y[{row},{col}] = {got:?}, expected {expected:?}"
        );
    }

    // Frozen direct OpenDSS references. Generated with OpenDSSDirect.py 0.9.4,
    // DSS-Python 0.15.7, C-API backend 0.14.5 (SVN 3723), using:
    // phases=3, buses=[hv lv], kvs=[12.47 .48], kvas=[300 300],
    // %Rs=[.5 .5], XHL=6, taps=[1.03 .97], %noloadloss=0, %imag=0,
    // ppm_antifloat=0 and RNeut=-1 on both windings.  The reference keys
    // are the terminal order used by these tests. Full matrices and reports
    // are recorded under docs/evidence/mc-pf-yprim; these entries pin each
    // connection's actual admittance
    // values, including the delta phase incidence and tap scaling.
    #[test]
    fn direct_dss_golden_yy_with_fixed_taps() {
        let p = prepare_transformer(&dss_golden_transformer(
            DistWindingConn::Wye,
            DistWindingConn::Wye,
        ))
        .unwrap();
        assert_entry(&p, 0, 0, 0.004914871575352249, -0.029489229452113495);
        assert_entry(&p, 0, 4, -0.13558226374781318, 0.8134935824868792);
        assert_entry(&p, 3, 3, 0.014744614726056746, -0.08846768835634049);
        assert_entry(&p, 4, 4, 3.7401893337699468, -22.441136002619682);
        assert_entry(&p, 4, 7, -3.7401893337699468, 22.441136002619682);
        assert_entry(&p, 7, 7, 11.22056800130984, -67.32340800785904);
    }

    #[test]
    fn direct_dss_golden_dd_with_fixed_taps() {
        let p = prepare_transformer(&dss_golden_transformer(
            DistWindingConn::Delta,
            DistWindingConn::Delta,
        ))
        .unwrap();
        assert_entry(&p, 0, 0, 0.0032765810502348334, -0.019659486301409002);
        assert_entry(&p, 0, 1, -0.0016382905251174167, 0.009829743150704501);
        assert_entry(&p, 0, 3, -0.09038817583187549, 0.542329054991253);
        assert_entry(&p, 0, 4, 0.04519408791593774, -0.2711645274956265);
        assert_entry(&p, 3, 3, 2.493459555846632, -14.960757335079794);
        assert_entry(&p, 3, 4, -1.246729777923316, 7.480378667539897);
    }

    #[test]
    fn direct_dss_golden_yd_with_fixed_taps() {
        let p = prepare_transformer(&dss_golden_transformer(
            DistWindingConn::Wye,
            DistWindingConn::Delta,
        ))
        .unwrap();
        assert_entry(&p, 0, 0, 0.004914871575352249, -0.029489229452113495);
        assert_entry(&p, 0, 3, -0.004914871575352249, 0.029489229452113495);
        assert_entry(&p, 0, 4, -0.0782784564721388, 0.4696707388328328);
        assert_entry(&p, 3, 3, 0.014744614726056746, -0.08846768835634049);
        assert_entry(&p, 4, 4, 2.493459555846632, -14.960757335079794);
        assert_entry(&p, 4, 5, -1.246729777923316, 7.480378667539897);
    }

    #[test]
    fn direct_dss_golden_dy_with_fixed_taps() {
        let p = prepare_transformer(&dss_golden_transformer(
            DistWindingConn::Delta,
            DistWindingConn::Wye,
        ))
        .unwrap();
        assert_entry(&p, 0, 0, 0.0032765810502348334, -0.019659486301409002);
        assert_entry(&p, 0, 1, -0.0016382905251174167, 0.009829743150704501);
        assert_entry(&p, 0, 3, -0.0782784564721388, 0.4696707388328328);
        assert_entry(&p, 0, 4, 0.0782784564721388, -0.4696707388328328);
        assert_entry(&p, 3, 3, 3.7401893337699468, -22.441136002619682);
        assert_entry(&p, 3, 6, -3.7401893337699468, 22.441136002619682);
    }

    #[test]
    fn direct_dss_golden_n_winding_rolls_pin_each_delta_incidence() {
        let plus_minus = prepare_transformer(&dss_golden_n_winding(1, -1)).unwrap();
        assert_entry(
            &plus_minus,
            0,
            0,
            0.0032765810502348334,
            -0.019659486301409002,
        );
        assert_entry(&plus_minus, 0, 3, -0.04519408791593774, 0.2711645274956265);
        assert_entry(&plus_minus, 0, 4, -0.04519408791593774, 0.2711645274956265);
        assert_entry(&plus_minus, 1, 3, 0.09038817583187549, -0.542329054991253);
        let minus_plus = prepare_transformer(&dss_golden_n_winding(-1, 1)).unwrap();
        assert_entry(&minus_plus, 0, 3, -0.04519408791593774, 0.2711645274956265);
        assert_entry(&minus_plus, 0, 4, 0.09038817583187549, -0.542329054991253);
        assert_entry(&minus_plus, 1, 3, -0.04519408791593774, 0.2711645274956265);
    }

    #[test]
    fn yy_has_zero_sum_rows_and_finite_leakage() {
        let p =
            prepare_transformer(&transformer(DistWindingConn::Wye, DistWindingConn::Wye)).unwrap();
        assert_eq!(p.terminals.len(), 8);
        for row in &p.y_prim {
            let sum: Complex64 = row.iter().copied().sum();
            assert!(sum.norm() < 1e-12, "row sum {sum:?}");
        }
        assert!(p
            .y_prim
            .iter()
            .flatten()
            .all(|z| z.re.is_finite() && z.im.is_finite()));
    }

    #[test]
    fn mixed_connection_couples_delta_phases() {
        let p = prepare_transformer(&transformer(DistWindingConn::Wye, DistWindingConn::Delta))
            .unwrap();
        assert_eq!(p.delta_direction, 1);
        // Delta terminal 1 is coupled to terminal 2 through its coil.
        let i = p
            .terminals
            .iter()
            .position(|t| t.bus == "lv" && t.terminal == "1")
            .unwrap();
        let j = p
            .terminals
            .iter()
            .position(|t| t.bus == "lv" && t.terminal == "2")
            .unwrap();
        assert!(p.y_prim[i][j].norm() > 0.0);
    }

    #[test]
    fn all_two_winding_connection_pairs_have_physical_finite_yprim() {
        for (from, to) in [
            (DistWindingConn::Wye, DistWindingConn::Wye),
            (DistWindingConn::Wye, DistWindingConn::Delta),
            (DistWindingConn::Delta, DistWindingConn::Wye),
            (DistWindingConn::Delta, DistWindingConn::Delta),
        ] {
            let primitive = prepare_transformer(&transformer(from, to)).unwrap();
            assert!(primitive
                .y_prim
                .iter()
                .flatten()
                .all(|z| z.re.is_finite() && z.im.is_finite()));
            for row in &primitive.y_prim {
                let sum: Complex64 = row.iter().copied().sum();
                assert!(sum.norm() < 1e-10, "{from:?}/{to:?} row sum {sum:?}");
            }
        }
    }

    #[test]
    fn two_winding_n_winding_uses_each_delta_roll_in_the_coil_map() {
        let mut opposite = transformer(DistWindingConn::Delta, DistWindingConn::Delta);
        opposite
            .extras
            .insert("bmopf_subtype".into(), Value::String("n_winding".into()));
        opposite.extras.insert(
            "bmopf_delta_rolls".into(),
            serde_json::json!({"1": 1, "2": -1}),
        );
        let mut aligned = opposite.clone();
        aligned.extras.insert(
            "bmopf_delta_rolls".into(),
            serde_json::json!({"1": 1, "2": 1}),
        );
        let p_opposite = prepare_transformer(&opposite).unwrap();
        let p_aligned = prepare_transformer(&aligned).unwrap();
        assert!(p_opposite
            .y_prim
            .iter()
            .zip(&p_aligned.y_prim)
            .flat_map(|(a, b)| a.iter().zip(b))
            .any(|(a, b)| (*a - *b).norm() > 1e-12));
    }

    #[test]
    fn ideal_transformer_is_rejected() {
        let mut t = transformer(DistWindingConn::Wye, DistWindingConn::Wye);
        t.xsc_pct[0] = 0.0;
        t.windings.iter_mut().for_each(|w| w.r_pct = 0.0);
        assert!(matches!(
            prepare_transformer(&t),
            Err(TransformerError::Unsupported { .. })
        ));
    }

    #[test]
    fn no_load_shunt_is_stamped_per_delta_coil() {
        let mut t = transformer(DistWindingConn::Wye, DistWindingConn::Delta);
        t.extras.insert(
            "no_load_shunt".into(),
            serde_json::json!({"winding": 2, "g": 0.001, "b": -0.002}),
        );
        let p = prepare_transformer(&t).unwrap();
        let lv: Vec<_> = p
            .terminals
            .iter()
            .enumerate()
            .filter(|(_, t)| t.bus == "lv")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(lv.len(), 3);
        for &i in &lv {
            assert!((p.y_shunt[i][i] - Complex64::new(0.002, -0.004)).norm() < 1e-14);
        }
        for a in 0..3 {
            let i = lv[a];
            let j = lv[(a + 1) % 3];
            assert!((p.y_shunt[i][j] - Complex64::new(-0.001, 0.002)).norm() < 1e-14);
        }
    }

    #[test]
    fn unequal_winding_ratings_are_referred_to_winding_one_base() {
        let mut t = transformer(DistWindingConn::Wye, DistWindingConn::Wye);
        t.windings[1].s_rating = 600_000.0;
        let p = prepare_transformer(&t).unwrap();
        let z = Complex64::new(1.0, 0.0) / p.y_1volt[0][0];
        let expected = Complex64::new(
            3.0 / 300_000.0 * (0.005 + 0.005 * 300_000.0 / 600_000.0),
            3.0 / 300_000.0 * 0.06,
        );
        assert!((z - expected).norm() < 1e-14, "{z:?} != {expected:?}");
    }

    fn one_phase_transformer() -> DistTransformer {
        let mut w1 = DistWinding::new(
            "hv",
            vec!["p".into(), "n".into()],
            DistWindingConn::Wye,
            100.0,
            1_000.0,
        );
        w1.r_pct = 1.0;
        let mut w2 = DistWinding::new(
            "lv",
            vec!["p".into(), "n".into()],
            DistWindingConn::Wye,
            10.0,
            1_000.0,
        );
        w2.r_pct = 1.0;
        DistTransformer::new("one", vec![w1, w2], vec![0.0], 1)
    }

    #[test]
    fn one_phase_common_base_matches_direct_oracle() {
        let p = prepare_transformer(&one_phase_transformer()).unwrap();
        let hv_p = p
            .terminals
            .iter()
            .position(|x| x.bus == "hv" && x.terminal == "p")
            .unwrap();
        let hv_n = p
            .terminals
            .iter()
            .position(|x| x.bus == "hv" && x.terminal == "n")
            .unwrap();
        let lv_p = p
            .terminals
            .iter()
            .position(|x| x.bus == "lv" && x.terminal == "p")
            .unwrap();
        let lv_n = p
            .terminals
            .iter()
            .position(|x| x.bus == "lv" && x.terminal == "n")
            .unwrap();
        assert!((p.y_prim[hv_p][hv_p] - Complex64::new(5.0, 0.0)).norm() < 1e-12);
        assert!((p.y_prim[hv_p][lv_p] + Complex64::new(5.0e1, 0.0)).norm() < 1e-12);
        assert!((p.y_prim[lv_p][lv_p] - Complex64::new(5.0e2, 0.0)).norm() < 1e-10);
        assert!((p.y_prim[hv_p][hv_n] + Complex64::new(5.0, 0.0)).norm() < 1e-12);
        assert!((p.y_prim[lv_p][lv_n] + Complex64::new(5.0e2, 0.0)).norm() < 1e-10);
    }

    #[test]
    fn fixed_tap_scales_the_referred_terminal_matrix() {
        let mut t = one_phase_transformer();
        t.windings[1].tap = 2.0;
        let p = prepare_transformer(&t).unwrap();
        let hv_p = p
            .terminals
            .iter()
            .position(|x| x.bus == "hv" && x.terminal == "p")
            .unwrap();
        let lv_p = p
            .terminals
            .iter()
            .position(|x| x.bus == "lv" && x.terminal == "p")
            .unwrap();
        assert!((p.y_prim[hv_p][hv_p] - Complex64::new(5.0, 0.0)).norm() < 1e-12);
        assert!((p.y_prim[hv_p][lv_p] + Complex64::new(2.5e1, 0.0)).norm() < 1e-12);
        assert!((p.y_prim[lv_p][lv_p] - Complex64::new(1.25e2, 0.0)).norm() < 1e-10);
    }

    #[test]
    fn solid_neutral_is_returned_as_an_exact_ground_constraint() {
        let mut t = one_phase_transformer();
        t.windings[0].r_neutral = Some(0.0);
        t.windings[0].x_neutral = Some(0.0);
        let p = prepare_transformer(&t).unwrap();
        assert!(p
            .grounded_terminals
            .iter()
            .any(|x| x.bus == "hv" && x.terminal == "n"));
    }

    #[test]
    fn wye_phase_permutation_keeps_the_declared_last_neutral_physical() {
        let mut t = transformer(DistWindingConn::Wye, DistWindingConn::Wye);
        t.windings[0].terminal_map = vec!["b".into(), "c".into(), "a".into(), "n".into()];
        t.windings[1].terminal_map = vec!["c".into(), "a".into(), "b".into(), "m".into()];
        t.windings[0].r_neutral = Some(0.0);
        t.windings[0].x_neutral = Some(0.0);
        let p = prepare_transformer(&t).unwrap();
        let hv_b = p
            .terminals
            .iter()
            .position(|x| x.bus == "hv" && x.terminal == "b")
            .unwrap();
        let lv_c = p
            .terminals
            .iter()
            .position(|x| x.bus == "lv" && x.terminal == "c")
            .unwrap();
        assert!(
            (p.y_prim[hv_b][lv_c] - Complex64::new(-0.13546023971044013, 0.8127614382626409))
                .norm()
                < 1e-12,
            "got {:?}",
            p.y_prim[hv_b][lv_c]
        );
        assert!(p
            .grounded_terminals
            .iter()
            .any(|x| x.bus == "hv" && x.terminal == "n"));
    }

    #[test]
    fn negative_neutral_resistance_is_the_floating_sentinel() {
        let mut t = one_phase_transformer();
        t.windings[0].r_neutral = Some(-1.0);
        t.windings[0].x_neutral = Some(0.0);
        let p = prepare_transformer(&t).unwrap();
        let neutral = p
            .terminals
            .iter()
            .position(|x| x.bus == "hv" && x.terminal == "n")
            .unwrap();
        assert!(p.y_series[neutral][neutral].norm() < 1.0e5);
    }
}
