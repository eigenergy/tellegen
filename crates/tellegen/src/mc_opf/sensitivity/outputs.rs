//! Exact gradients of physical measurements, independent of power lift columns.
use super::*;
use crate::mc_opf::model::{Affine, Pair};

fn gradient(a: &Affine, n: usize) -> Vec<f64> {
    let mut g = vec![0.; n];
    for (&i, &v) in &a.terms {
        g[i] = v;
    }
    g
}
fn difference(v: &[Pair], p: usize, n: Option<usize>) -> Pair {
    std::array::from_fn(|k| n.map_or_else(|| v[p][k].clone(), |n| v[p][k].sub(&v[n][k])))
}
fn power_gradient(v: &Pair, i: &Pair, x: &[f64], reactive: bool) -> Vec<f64> {
    let terms = if reactive {
        [(1, 0, 1.), (0, 1, -1.)]
    } else {
        [(0, 0, 1.), (1, 1, 1.)]
    };
    let mut g = vec![0.; x.len()];
    for (a, b, sign) in terms {
        let (va, ib) = (v[a].value(x), i[b].value(x));
        for (&c, &d) in &v[a].terms {
            g[c] += sign * d * ib;
        }
        for (&c, &d) in &i[b].terms {
            g[c] += sign * d * va;
        }
    }
    g
}
impl McOpfSensitivities<'_> {
    pub(super) fn output_gradient(
        &self,
        output: &McOpfSensitivityOutput,
    ) -> Result<(Vec<f64>, McOpfOutputUnit)> {
        let m = &self.model;
        let p = &m.prep;
        let x = &self.result.duals.as_ref().unwrap().primal;
        let vb = p.bases.voltage_base_v;
        let sb = p.bases.power_base_va;
        let unsupported = || {
            error(
                ErrorKind::UnsupportedOutput,
                format!("unknown or nondifferentiable output {output:?}"),
            )
        };
        let (mut g, scale, unit) = match output {
            McOpfSensitivityOutput::Objective => {
                return Ok((
                    self.objective_gradient.clone(),
                    McOpfOutputUnit::CurrencyPerHour,
                ))
            }
            McOpfSensitivityOutput::Voltage { terminal, part } => {
                let k = p
                    .terminals
                    .iter()
                    .position(|t| t.bus == terminal.bus && t.terminal == terminal.terminal)
                    .ok_or_else(unsupported)?;
                let v = &m.voltage[k];
                match part {
                    McOpfVoltagePart::Real => (gradient(&v[0], x.len()), vb, McOpfOutputUnit::Volt),
                    McOpfVoltagePart::Imaginary => {
                        (gradient(&v[1], x.len()), vb, McOpfOutputUnit::Volt)
                    }
                    McOpfVoltagePart::Magnitude | McOpfVoltagePart::Angle => {
                        let (re, im) = (v[0].value(x), v[1].value(x));
                        let norm = re.hypot(im);
                        // Magnitude and angle have no unique derivative at zero.
                        if norm <= 1e-12 {
                            return Err(unsupported());
                        }
                        let (a, b, scale, unit) = if *part == McOpfVoltagePart::Magnitude {
                            (re / norm, im / norm, vb, McOpfOutputUnit::Volt)
                        } else {
                            (
                                -im / (norm * norm),
                                re / (norm * norm),
                                1.,
                                McOpfOutputUnit::Radian,
                            )
                        };
                        let v = v[0].scale(a).add(&v[1].scale(b));
                        (gradient(&v, x.len()), scale, unit)
                    }
                }
            }
            McOpfSensitivityOutput::Device {
                kind,
                identity,
                axis,
                quantity,
            } => {
                let k = p
                    .devices
                    .iter()
                    .position(|d| d.kind == *kind && d.identity == *identity)
                    .ok_or_else(unsupported)?;
                let dev = &p.devices[k];
                let cols = &m.devices[k];
                let (v, i) = match axis {
                    McOpfDeviceAxis::Coil { index } => {
                        let coil = dev.coils.get(*index).ok_or_else(unsupported)?;
                        (
                            difference(&m.voltage, coil.positive, coil.negative),
                            cols.current[*index].clone(),
                        )
                    }
                    McOpfDeviceAxis::Terminal { terminal } => {
                        let node = *dev
                            .terminals
                            .iter()
                            .find(|&&n| p.terminals[n].terminal == *terminal)
                            .ok_or_else(unsupported)?;
                        let mut current = [Affine::default(), Affine::default()];
                        for (coil, i) in dev.coils.iter().zip(&cols.current) {
                            let sign = f64::from(coil.positive == node)
                                - f64::from(coil.negative == Some(node));
                            for k in 0..2 {
                                current[k] = current[k].add(&i[k].scale(sign));
                            }
                        }
                        (m.voltage[node].clone(), current)
                    }
                };
                flow(&v, &i, x, *quantity, sb, vb)
            }
            McOpfSensitivityOutput::Branch {
                identity,
                end,
                conductor,
                quantity,
            } => {
                let k = p
                    .branches
                    .iter()
                    .position(|b| b.identity == *identity)
                    .ok_or_else(unsupported)?;
                let br = &p.branches[k];
                let (nodes, g, b, sign) = match end {
                    McOpfBranchEnd::From => (&br.from, &br.g_from, &br.b_from, 1.),
                    McOpfBranchEnd::To => (&br.to, &br.g_to, &br.b_to, -1.),
                };
                let c = *conductor;
                let node = *nodes.get(c).ok_or_else(unsupported)?;
                let mut current = [Affine::default(), Affine::default()];
                if !br.open {
                    for (part, i) in current.iter_mut().zip(&m.branch_current[k][c]) {
                        *part = i.scale(sign);
                    }
                    for (j, &n) in nodes.iter().enumerate() {
                        current[0] = current[0]
                            .add(&m.voltage[n][0].scale(g[c][j]))
                            .sub(&m.voltage[n][1].scale(b[c][j]));
                        current[1] = current[1]
                            .add(&m.voltage[n][0].scale(b[c][j]))
                            .add(&m.voltage[n][1].scale(g[c][j]));
                    }
                }
                flow(&m.voltage[node], &current, x, *quantity, sb, vb)
            }
        };
        for v in &mut g {
            *v *= scale;
            finite(*v)?;
        }
        Ok((g, unit))
    }
}
fn flow(
    v: &Pair,
    i: &Pair,
    x: &[f64],
    q: McOpfFlowQuantity,
    sb: f64,
    vb: f64,
) -> (Vec<f64>, f64, McOpfOutputUnit) {
    match q {
        McOpfFlowQuantity::CurrentReal => {
            (gradient(&i[0], x.len()), sb / vb, McOpfOutputUnit::Ampere)
        }
        McOpfFlowQuantity::CurrentImaginary => {
            (gradient(&i[1], x.len()), sb / vb, McOpfOutputUnit::Ampere)
        }
        McOpfFlowQuantity::ActivePower => {
            (power_gradient(v, i, x, false), sb, McOpfOutputUnit::Watt)
        }
        McOpfFlowQuantity::ReactivePower => {
            (power_gradient(v, i, x, true), sb, McOpfOutputUnit::Var)
        }
    }
}
