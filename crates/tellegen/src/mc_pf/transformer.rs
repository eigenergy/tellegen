//! Finite-leakage OpenDSS transformer primitive for the multiconductor solver.
//!
//! The implementation follows the Version 8 OpenDSS `CalcY_Terminal` path:
//! winding leakage is assembled as `ZB`, inverted on the one-volt winding
//! base, transformed by the `[-1 I]` winding-difference map, then referred to
//! the physical coil terminals.  The final terminal matrix is assembled with
//! the WYE/DELTA incidence map, so a delta winding is represented by its
//! actual line-to-line coils rather than by independent phase ratios.
//!
//! This module deliberately prepares finite-leakage two-winding elements and
//! the three coupled windings produced by PowerIO for a BMOPF centre tap.
//! Ideal transformers need voltage constraints and are rejected here; silently
//! returning a zero admittance would disconnect the network. Other arbitrary
//! multiwinding data remains unsupported.

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

/// Prepare one finite-leakage transformer from the canonical PowerIO
/// representation. Besides ordinary two-winding units, this accepts only the
/// three-winding shape tagged by PowerIO as a BMOPF `center_tap` transformer.
pub fn prepare_transformer(
    transformer: &DistTransformer,
) -> Result<TransformerPrimitive, TransformerError> {
    if transformer
        .extras
        .get("bmopf_subtype")
        .and_then(Value::as_str)
        == Some("single_phase_autotransformer")
    {
        return prepare_single_phase_autotransformer(transformer);
    }
    validate_shape(transformer)?;
    let phases = transformer.phases;
    let name = transformer.name.clone();

    let direction = delta_direction(transformer);
    let directions: Vec<_> = transformer
        .windings
        .iter()
        .enumerate()
        .map(|(index, _)| delta_direction_for(transformer, index, direction))
        .collect();
    let vbase: Vec<_> = transformer
        .windings
        .iter()
        .map(|winding| coil_voltage(winding, phases))
        .collect();
    let y_1volt = winding_admittance(transformer)?;

    let mut terminals = Vec::new();
    let mut y_series = Vec::new();
    let mut y_shunt = Vec::new();
    let mut grounded_terminals = Vec::new();
    // Build the unique terminal list once. Coils from all phases then stamp
    // through the same physical neutral column when a neutral is explicit.
    for winding in &transformer.windings {
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
        let mut b = vec![vec![Complex64::new(0.0, 0.0); nterm]; transformer.windings.len()];
        for (index, winding) in transformer.windings.iter().enumerate() {
            let (a, z) = coil_refs(winding, phase, phases, directions[index], &terminals, &name)?;
            let k = Complex64::new(1.0 / (vbase[index] * checked_tap(winding, &name)?), 0.0);
            if let Some(i) = a {
                b[index][i] += k;
            }
            if let Some(i) = z {
                b[index][i] -= k;
            }
        }
        for i in 0..nterm {
            for j in 0..nterm {
                let mut value = Complex64::new(0.0, 0.0);
                for p in 0..transformer.windings.len() {
                    for q in 0..transformer.windings.len() {
                        value += b[p][i] * y_1volt[p][q] * b[q][j];
                    }
                }
                y_series[i][j] += value;
            }
        }

        // Explicit neutral impedance is one shunt from the physical WYE
        // neutral conductor to ground, shared by all phase coils.
        if phase == 0 {
            for (idx, winding) in transformer.windings.iter().enumerate() {
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

/// Build OpenDSS's winding-domain `Y_1Volt` matrix. Pairwise short-circuit
/// impedances are first converted to the reduced `ZB` matrix relative to the
/// final winding, then lifted back through winding-difference incidence. For a
/// two-winding unit this reduces exactly to `y * [[1,-1],[-1,1]]`.
fn winding_admittance(t: &DistTransformer) -> Result<ComplexMatrix, TransformerError> {
    let count = t.windings.len();
    // `validate_shape` runs first and admits two windings, or three for a
    // tagged centre tap. Keep that contract local so a new caller cannot
    // reach the `unreachable!` in `pair_index` by skipping validation.
    debug_assert!(
        (2..=3).contains(&count),
        "winding_admittance expects a validated two- or three-winding transformer"
    );
    let reference = count - 1;
    let zbase = t.phases as f64 / t.windings[0].s_rating;
    let r_common: Vec<_> = t
        .windings
        .iter()
        .map(|winding| winding.r_pct / 100.0 * t.windings[0].s_rating / winding.s_rating)
        .collect();
    let pair_index = |left: usize, right: usize| match (left.min(right), left.max(right)) {
        (0, 1) => 0,
        (0, 2) => 1,
        (1, 2) => 2,
        _ => unreachable!("shape validation permits at most three windings"),
    };
    let pair_impedance = |left: usize, right: usize| {
        Complex64::new(
            (r_common[left] + r_common[right]) * zbase,
            t.xsc_pct[pair_index(left, right)] / 100.0 * zbase,
        )
    };

    let mut zb = vec![vec![Complex64::default(); reference]; reference];
    for (i, row) in zb.iter_mut().enumerate() {
        for (j, value) in row.iter_mut().enumerate() {
            let zi_ref = pair_impedance(i, reference);
            let zj_ref = pair_impedance(j, reference);
            *value = if i == j {
                zi_ref
            } else {
                (zi_ref + zj_ref - pair_impedance(i, j)) / 2.0
            };
        }
    }
    let yb = invert_reduced_leakage(&zb, &t.name)?;
    let mut result = vec![vec![Complex64::default(); count]; count];
    // A maps winding voltages to differences against the final winding:
    // row i is `u_i - u_reference`. Y_1Volt = A^T inv(ZB) A.
    for i in 0..reference {
        for j in 0..reference {
            let value = yb[i][j];
            result[i][j] += value;
            result[i][reference] -= value;
            result[reference][j] -= value;
            result[reference][reference] += value;
        }
    }
    Ok(result)
}

fn invert_reduced_leakage(
    matrix: &ComplexMatrix,
    name: &str,
) -> Result<ComplexMatrix, TransformerError> {
    let finite = |value: Complex64| value.re.is_finite() && value.im.is_finite();
    match matrix.len() {
        1 if matrix[0].len() == 1 => {
            let z = matrix[0][0];
            if !finite(z) || z.norm() == 0.0 {
                return Err(TransformerError::SingularLeakage {
                    transformer: name.to_owned(),
                });
            }
            Ok(vec![vec![z.recip()]])
        }
        2 if matrix.iter().all(|row| row.len() == 2) => {
            let determinant = matrix[0][0] * matrix[1][1] - matrix[0][1] * matrix[1][0];
            // Only exact singularity is rejected, matching the two-winding
            // path: an absolute pivot cutoff would refuse legitimate
            // high-rating units whose 1 V-base impedances sit far below any
            // fixed threshold. A nearly singular reduced matrix, such as two
            // half windings with identical pairwise impedances to the
            // primary, therefore inverts with reduced accuracy rather than
            // failing here; the solver's KCL residual check reports it.
            if matrix.iter().flatten().any(|value| !finite(*value))
                || !finite(determinant)
                || determinant.norm() == 0.0
            {
                return Err(TransformerError::SingularLeakage {
                    transformer: name.to_owned(),
                });
            }
            Ok(vec![
                vec![matrix[1][1] / determinant, -matrix[0][1] / determinant],
                vec![-matrix[1][0] / determinant, matrix[0][0] / determinant],
            ])
        }
        _ => Err(TransformerError::Unsupported {
            transformer: name.to_owned(),
            reason: "only two-winding and BMOPF centre-tap leakage matrices are supported".into(),
        }),
    }
}

/// Prepare the fixed-ratio BMOPF single-phase autotransformer/regulator.
///
/// Unlike an isolating transformer, this subtype states its impedance in
/// referred ohms and its ratio as regulated/source voltage.  Its winding
/// primitive is therefore `y * [1, -n; -n, n^2]`, where `n = 1/a` for the
/// default ANSI Type B connection and `n = a` for Type A.  This matches the
/// BMOPFTools calculation contract and the equivalent fixed-tap OpenDSS
/// two-winding regulator.
fn prepare_single_phase_autotransformer(
    transformer: &DistTransformer,
) -> Result<TransformerPrimitive, TransformerError> {
    let name = transformer.name.clone();
    if transformer.phases != 1 || transformer.windings.len() != 2 {
        return Err(TransformerError::Unsupported {
            transformer: name,
            reason: format!(
                "single_phase_autotransformer requires one phase and two windings; got {} phases and {} windings",
                transformer.phases,
                transformer.windings.len()
            ),
        });
    }
    let from = &transformer.windings[0];
    let to = &transformer.windings[1];
    for (idx, winding) in [from, to].iter().enumerate() {
        if winding.conn != DistWindingConn::Wye || winding.terminal_map.len() != 2 {
            return Err(TransformerError::Invalid {
                transformer: transformer.name.clone(),
                field: format!("windings[{idx}].terminal_map"),
                reason: "autotransformer winding requires phase and reference terminals".into(),
            });
        }
    }

    let tap_ratio = checked_tap(to, &transformer.name)?;
    let regulator_type = transformer
        .extras
        .get("regulator_type")
        .and_then(Value::as_str)
        .unwrap_or("B")
        .trim()
        .to_ascii_uppercase();
    let n_eff = match regulator_type.as_str() {
        "A" => tap_ratio,
        "B" => 1.0 / tap_ratio,
        _ => {
            return Err(TransformerError::Invalid {
                transformer: transformer.name.clone(),
                field: "regulator_type".into(),
                reason: format!("expected A or B, got `{regulator_type}`"),
            });
        }
    };
    let impedance = |key: &str| -> Result<f64, TransformerError> {
        let value = value_number(&transformer.extras, key).unwrap_or(0.0);
        if !value.is_finite() || value < 0.0 {
            return Err(TransformerError::Invalid {
                transformer: transformer.name.clone(),
                field: key.into(),
                reason: "must be finite and nonnegative".into(),
            });
        }
        Ok(value)
    };
    let z = Complex64::new(
        impedance("r_series_from")? + n_eff.powi(2) * impedance("r_series_to")?,
        impedance("x_series_from")? + n_eff.powi(2) * impedance("x_series_to")?,
    );
    if z.norm() == 0.0 {
        return Err(TransformerError::Unsupported {
            transformer: transformer.name.clone(),
            reason: "zero series impedance requires an ideal regulator voltage constraint".into(),
        });
    }
    let y = z.recip();
    let y_1volt = vec![vec![y, -n_eff * y], vec![-n_eff * y, n_eff.powi(2) * y]];

    // Match the BMOPFTools primitive order: from phase, to phase, from
    // reference, to reference. Repeated physical terminals are coalesced.
    let mut terminals = Vec::new();
    for (winding, terminal) in [
        (from, &from.terminal_map[0]),
        (to, &to.terminal_map[0]),
        (from, &from.terminal_map[1]),
        (to, &to.terminal_map[1]),
    ] {
        if !terminals
            .iter()
            .any(|item: &TransformerTerminal| item.bus == winding.bus && item.terminal == *terminal)
        {
            terminals.push(TransformerTerminal {
                bus: winding.bus.clone(),
                terminal: terminal.clone(),
            });
        }
    }
    let mut from_incidence = vec![0.0; terminals.len()];
    let mut to_incidence = vec![0.0; terminals.len()];
    for (incidence, winding) in [(&mut from_incidence, from), (&mut to_incidence, to)] {
        let phase = terminal_index(&terminals, &winding.bus, &winding.terminal_map[0]).unwrap();
        let reference = terminal_index(&terminals, &winding.bus, &winding.terminal_map[1]).unwrap();
        incidence[phase] += 1.0;
        incidence[reference] -= 1.0;
    }
    let winding_difference: Vec<f64> = from_incidence
        .iter()
        .zip(&to_incidence)
        .map(|(from, to)| from - n_eff * to)
        .collect();
    let mut y_series = vec![vec![Complex64::default(); terminals.len()]; terminals.len()];
    for i in 0..terminals.len() {
        for j in 0..terminals.len() {
            y_series[i][j] = y * winding_difference[i] * winding_difference[j];
        }
    }
    let mut y_shunt = vec![vec![Complex64::default(); terminals.len()]; terminals.len()];
    add_excitation(transformer, &mut y_shunt, &terminals)?;
    let mut y_prim = y_series.clone();
    for i in 0..terminals.len() {
        for j in 0..terminals.len() {
            y_prim[i][j] += y_shunt[i][j];
        }
    }
    Ok(TransformerPrimitive {
        name: transformer.name.clone(),
        terminals,
        y_series,
        y_shunt,
        y_prim,
        grounded_terminals: Vec::new(),
        delta_direction: 1,
        y_1volt,
    })
}

fn validate_shape(t: &DistTransformer) -> Result<(), TransformerError> {
    let name = t.name.clone();
    let center_tap = t.extras.get("bmopf_subtype").and_then(Value::as_str) == Some("center_tap");
    let expected_windings = if center_tap { 3 } else { 2 };
    if t.windings.len() != expected_windings {
        return Err(TransformerError::Unsupported {
            transformer: name,
            reason: format!(
                "{} windings are present; expected {expected_windings} for {}",
                t.windings.len(),
                if center_tap {
                    "a BMOPF centre-tap primitive"
                } else {
                    "a finite two-winding primitive"
                }
            ),
        });
    }
    if center_tap && t.phases != 1 {
        return Err(TransformerError::Unsupported {
            transformer: t.name.clone(),
            reason: format!(
                "BMOPF centre-tap primitives require one phase; got {}",
                t.phases
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
    let expected_xsc = if center_tap { 3 } else { 1 };
    if t.xsc_pct.len() != expected_xsc {
        return Err(TransformerError::Invalid {
            transformer: t.name.clone(),
            field: "xsc_pct".into(),
            reason: format!(
                "{} requires exactly {expected_xsc} pairwise short-circuit values",
                if center_tap {
                    "a centre-tap transformer"
                } else {
                    "a two-winding transformer"
                }
            ),
        });
    }
    if t.xsc_pct
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(TransformerError::Invalid {
            transformer: t.name.clone(),
            field: "xsc_pct".into(),
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
    let explicit_ideal =
        t.xsc_pct.iter().all(|value| *value == 0.0) && t.windings.iter().all(|w| w.r_pct == 0.0);
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
        if winding == 0 || winding as usize > t.windings.len() {
            return Err(TransformerError::Invalid {
                transformer: name.clone(),
                field: "no_load_shunt.winding".into(),
                reason: format!("must be between 1 and {}", t.windings.len()),
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
        // The tagged autotransformer fields are from-winding per-coil
        // siemens. Other raw BMOPF subtypes require `no_load_shunt` during
        // preflight, so their placement reaches this function explicitly.
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

    fn autotransformer(regulator_type: &str, tap_ratio: f64) -> DistTransformer {
        let from = DistWinding::new(
            "src",
            vec!["p".into(), "n".into()],
            DistWindingConn::Wye,
            2_400.0,
            500_000.0,
        );
        let mut to = DistWinding::new(
            "reg",
            vec!["p".into(), "n".into()],
            DistWindingConn::Wye,
            2_400.0,
            500_000.0,
        );
        to.tap = tap_ratio;
        let mut transformer = DistTransformer::new("reg", vec![from, to], vec![0.0], 1);
        transformer.extras.insert(
            "bmopf_subtype".into(),
            Value::String("single_phase_autotransformer".into()),
        );
        transformer.extras.insert(
            "regulator_type".into(),
            Value::String(regulator_type.into()),
        );
        transformer
            .extras
            .insert("r_series_from".into(), serde_json::json!(0.5));
        transformer
            .extras
            .insert("x_series_from".into(), serde_json::json!(2.0));
        transformer
            .extras
            .insert("r_series_to".into(), serde_json::json!(0.25));
        transformer
            .extras
            .insert("x_series_to".into(), serde_json::json!(0.5));
        transformer
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
    fn type_b_autotransformer_matches_bmopf_winding_primitive() {
        let tap = 1.05;
        let n_eff = 1.0 / tap;
        let p = prepare_transformer(&autotransformer("B", tap)).unwrap();
        let z = Complex64::new(0.5 + n_eff * n_eff * 0.25, 2.0 + n_eff * n_eff * 0.5);
        let y = z.recip();
        assert_eq!(p.terminals.len(), 4);
        assert!((p.y_prim[0][0] - y).norm() < 1e-12);
        assert!((p.y_prim[0][1] + n_eff * y).norm() < 1e-12);
        assert!((p.y_prim[1][1] - n_eff * n_eff * y).norm() < 1e-12);
        for row in &p.y_prim {
            assert!(row.iter().copied().sum::<Complex64>().norm() < 1e-12);
        }
    }

    #[test]
    fn type_a_autotransformer_uses_reciprocal_connection() {
        let tap = 1.05;
        let p = prepare_transformer(&autotransformer("A", tap)).unwrap();
        let z = Complex64::new(0.5 + tap * tap * 0.25, 2.0 + tap * tap * 0.5);
        let y = z.recip();
        assert!((p.y_prim[0][1] + tap * y).norm() < 1e-12);
        assert!((p.y_prim[1][1] - tap * tap * y).norm() < 1e-12);
    }

    #[test]
    fn autotransformer_no_load_shunt_spans_the_from_winding() {
        let mut transformer = autotransformer("B", 1.05);
        transformer
            .extras
            .insert("g_no_load".into(), serde_json::json!(0.001));
        transformer
            .extras
            .insert("b_no_load".into(), serde_json::json!(-0.002));
        let p = prepare_transformer(&transformer).unwrap();
        let shunt = Complex64::new(0.001, -0.002);
        assert!((p.y_shunt[0][0] - shunt).norm() < 1e-14);
        assert!((p.y_shunt[0][2] + shunt).norm() < 1e-14);
        assert!((p.y_shunt[2][0] + shunt).norm() < 1e-14);
        assert!((p.y_shunt[2][2] - shunt).norm() < 1e-14);
    }

    #[test]
    fn ideal_autotransformer_is_rejected_as_a_constraint() {
        let mut transformer = autotransformer("B", 1.05);
        for key in [
            "r_series_from",
            "x_series_from",
            "r_series_to",
            "x_series_to",
        ] {
            transformer
                .extras
                .insert(key.into(), serde_json::json!(0.0));
        }
        assert!(matches!(
            prepare_transformer(&transformer),
            Err(TransformerError::Unsupported { .. })
        ));
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

    fn center_tap_transformer() -> DistTransformer {
        let mut primary = DistWinding::new(
            "mv",
            vec!["1".into(), "2".into()],
            DistWindingConn::Wye,
            11_000.0,
            25_000.0,
        );
        primary.r_pct = 0.5;
        let mut leg_1 = DistWinding::new(
            "lv",
            vec!["1".into(), "4".into()],
            DistWindingConn::Wye,
            240.0,
            25_000.0,
        );
        leg_1.r_pct = 0.5;
        let mut leg_2 = DistWinding::new(
            "lv",
            vec!["4".into(), "2".into()],
            DistWindingConn::Wye,
            240.0,
            25_000.0,
        );
        leg_2.r_pct = 0.5;
        let mut transformer =
            DistTransformer::new("split", vec![primary, leg_1, leg_2], vec![4.0, 4.0, 4.0], 1);
        transformer
            .extras
            .insert("bmopf_subtype".into(), Value::String("center_tap".into()));
        transformer
    }

    #[test]
    fn center_tap_has_five_terminal_coupled_primitive_and_series_aiding_legs() {
        let p = prepare_transformer(&center_tap_transformer()).unwrap();
        let terminal_keys: Vec<_> = p
            .terminals
            .iter()
            .map(|terminal| (terminal.bus.as_str(), terminal.terminal.as_str()))
            .collect();
        assert_eq!(
            terminal_keys,
            vec![
                ("mv", "1"),
                ("mv", "2"),
                ("lv", "1"),
                ("lv", "4"),
                ("lv", "2")
            ]
        );
        assert_eq!(p.y_1volt.len(), 3);
        for row in &p.y_prim {
            assert!(row.iter().copied().sum::<Complex64>().norm() < 1e-10);
        }

        // The no-current voltage relationship is +11 kV across the primary,
        // +240 V from leg 1 to centre, and +240 V from centre to leg 2. Thus
        // the two outer secondary terminals are 180 degrees apart.
        let voltage = [
            Complex64::new(11_000.0, 0.0),
            Complex64::default(),
            Complex64::new(240.0, 0.0),
            Complex64::default(),
            Complex64::new(-240.0, 0.0),
        ];
        for row in &p.y_series {
            let current: Complex64 = row.iter().zip(voltage).map(|(y, v)| *y * v).sum();
            assert!(current.norm() < 1e-9, "no-load current {current:?}");
        }
    }

    #[test]
    fn center_tap_supports_fixed_tap_excitation_and_neutral_impedance() {
        let mut transformer = center_tap_transformer();
        transformer.windings[0].tap = 1.05;
        transformer.windings[1].r_neutral = Some(5.0);
        transformer.windings[1].x_neutral = Some(1.0);
        transformer.extras.insert(
            "no_load_shunt".into(),
            serde_json::json!({"winding": 2, "g": 0.001, "b": -0.002}),
        );
        let p = prepare_transformer(&transformer).unwrap();
        let primary = p
            .terminals
            .iter()
            .position(|terminal| terminal.bus == "mv" && terminal.terminal == "1")
            .unwrap();
        let center = p
            .terminals
            .iter()
            .position(|terminal| terminal.bus == "lv" && terminal.terminal == "4")
            .unwrap();
        let untapped = prepare_transformer(&center_tap_transformer()).unwrap();
        assert!((p.y_series[primary][primary] - untapped.y_series[primary][primary]).norm() > 1e-8);
        let mut ungrounded = transformer.clone();
        ungrounded.windings[1].r_neutral = None;
        ungrounded.windings[1].x_neutral = None;
        let ungrounded = prepare_transformer(&ungrounded).unwrap();
        assert!(
            (p.y_series[center][center]
                - ungrounded.y_series[center][center]
                - Complex64::new(5.0, 1.0).recip())
            .norm()
                < 1e-14
        );
        let leg_1 = p
            .terminals
            .iter()
            .position(|terminal| terminal.bus == "lv" && terminal.terminal == "1")
            .unwrap();
        assert!((p.y_shunt[leg_1][center] - Complex64::new(-0.001, 0.002)).norm() < 1e-14);

        let floating = prepare_transformer(&center_tap_transformer()).unwrap();
        assert!(floating.grounded_terminals.is_empty());
        let mut solid = center_tap_transformer();
        solid.windings[1].r_neutral = Some(0.0);
        solid.windings[1].x_neutral = Some(0.0);
        let solid = prepare_transformer(&solid).unwrap();
        assert!(solid
            .grounded_terminals
            .iter()
            .any(|terminal| terminal.bus == "lv" && terminal.terminal == "4"));
    }

    #[test]
    fn arbitrary_three_winding_transformer_remains_unsupported() {
        let mut transformer = center_tap_transformer();
        transformer.extras.clear();
        assert!(matches!(
            prepare_transformer(&transformer),
            Err(TransformerError::Unsupported { .. })
        ));
    }

    #[test]
    fn center_tap_fixed_tap_and_excitation_match_direct_opendss_yprim() {
        // Frozen from BMOPFTools' transformer_interoperability/center_tap.dss
        // using OpenDSSDirect.py 0.9.4, with winding-1 tap=1.06,
        // %noloadloss=.2, %imag=.4, and ppm_antifloat=0. Repeated centre-tap
        // rows are aggregated into the five physical PowerIO terminals.
        let mut transformer = center_tap_transformer();
        transformer.name = "tx".into();
        transformer.windings[0].bus = "f".into();
        transformer.windings[0].terminal_map = vec!["1".into(), "4".into()];
        transformer.windings[0].v_ref = 240.0;
        transformer.windings[0].s_rating = 10_000.0;
        transformer.windings[0].r_pct = 1.0;
        transformer.windings[0].tap = 1.06;
        for winding in &mut transformer.windings[1..] {
            winding.bus = "t".into();
            winding.v_ref = 120.0;
            winding.s_rating = 10_000.0;
            winding.r_pct = 1.0;
        }
        transformer.xsc_pct = vec![4.0, 4.0, 4.0];
        transformer.extras.insert(
            "no_load_shunt".into(),
            serde_json::json!({
                "winding": 2,
                "g": 0.001388888888888889,
                "b": -0.002777777777777778
            }),
        );
        let p = prepare_transformer(&transformer).unwrap();
        assert_entry(&p, 0, 0, 2.0601769444774067, -4.1203538889548135);
        assert_entry(&p, 0, 2, -2.1837875611460507, 4.367575122292101);
        assert_entry(&p, 0, 4, 2.183787561146052, -4.367575122292104);
        assert_entry(&p, 2, 2, 9.260648148148144, -18.52129629629629);
        assert_entry(&p, 2, 3, -13.890277777777772, 27.780555555555544);
        assert_entry(&p, 3, 3, 27.779166666666658, -55.558333333333316);
        assert_entry(&p, 4, 4, 9.259259259259258, -18.518518518518515);
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
