//! The linked DYR records of a case as named parameters, and edits of single
//! parameter values in the DYR text.
//!
//! A parameter editor needs no engine change: every model reads its values
//! from the DYR text when a session starts. The edit replaces exactly the
//! token of the edited parameter in the record's original text, so every
//! other byte of the file (comments, layout, records tellegen does not read)
//! stays as it was; the edited text is read back to check the edit.

use serde::{Deserialize, Serialize};

use powerio::{BalancedNetwork, DynamicCase, DynamicRecord};

/// One parameter of a PSS/E model: name, bucket, unit, description.
struct Param(&'static str, &'static str, &'static str, &'static str);

/// PSS/E parameters in record order (PSS/E Model Library) for the models
/// powerio reads (`powerio::SUPPORTED_MODELS`), clustered into buckets for an
/// editor.
const SCHEMA: &[(&str, &[Param])] = &[
    (
        "GENCLS",
        &[
            Param(
                "H",
                "Mechanics",
                "s",
                "inertia constant (MW·s/MVA on the machine base)",
            ),
            Param("D", "Mechanics", "pu", "speed damping"),
        ],
    ),
    (
        "GENROU",
        &[
            Param(
                "T'do",
                "Time constants",
                "s",
                "d-axis transient open-circuit time constant",
            ),
            Param(
                "T''do",
                "Time constants",
                "s",
                "d-axis subtransient open-circuit time constant",
            ),
            Param(
                "T'qo",
                "Time constants",
                "s",
                "q-axis transient open-circuit time constant",
            ),
            Param(
                "T''qo",
                "Time constants",
                "s",
                "q-axis subtransient open-circuit time constant",
            ),
            Param(
                "H",
                "Mechanics",
                "s",
                "inertia constant (MW·s/MVA on the machine base)",
            ),
            Param("D", "Mechanics", "pu", "speed damping"),
            Param("Xd", "Reactances", "pu", "d-axis synchronous reactance"),
            Param("Xq", "Reactances", "pu", "q-axis synchronous reactance"),
            Param("X'd", "Reactances", "pu", "d-axis transient reactance"),
            Param("X'q", "Reactances", "pu", "q-axis transient reactance"),
            Param(
                "X''d",
                "Reactances",
                "pu",
                "subtransient reactance (X''q = X''d)",
            ),
            Param("Xl", "Reactances", "pu", "stator leakage reactance"),
            Param(
                "S(1.0)",
                "Saturation",
                "",
                "saturation factor at 1.0 pu flux",
            ),
            Param(
                "S(1.2)",
                "Saturation",
                "",
                "saturation factor at 1.2 pu flux",
            ),
        ],
    ),
    (
        "GENROE",
        &[
            Param(
                "T'do",
                "Time constants",
                "s",
                "d-axis transient open-circuit time constant",
            ),
            Param(
                "T''do",
                "Time constants",
                "s",
                "d-axis subtransient open-circuit time constant",
            ),
            Param(
                "T'qo",
                "Time constants",
                "s",
                "q-axis transient open-circuit time constant",
            ),
            Param(
                "T''qo",
                "Time constants",
                "s",
                "q-axis subtransient open-circuit time constant",
            ),
            Param(
                "H",
                "Mechanics",
                "s",
                "inertia constant (MW·s/MVA on the machine base)",
            ),
            Param("D", "Mechanics", "pu", "speed damping"),
            Param("Xd", "Reactances", "pu", "d-axis synchronous reactance"),
            Param("Xq", "Reactances", "pu", "q-axis synchronous reactance"),
            Param("X'd", "Reactances", "pu", "d-axis transient reactance"),
            Param("X'q", "Reactances", "pu", "q-axis transient reactance"),
            Param(
                "X''d",
                "Reactances",
                "pu",
                "subtransient reactance (X''q = X''d)",
            ),
            Param("Xl", "Reactances", "pu", "stator leakage reactance"),
            Param(
                "S(1.0)",
                "Saturation",
                "",
                "saturation factor at 1.0 pu flux",
            ),
            Param(
                "S(1.2)",
                "Saturation",
                "",
                "saturation factor at 1.2 pu flux",
            ),
        ],
    ),
    (
        "GENSAL",
        &[
            Param(
                "T'do",
                "Time constants",
                "s",
                "d-axis transient open-circuit time constant",
            ),
            Param(
                "T''do",
                "Time constants",
                "s",
                "d-axis subtransient open-circuit time constant",
            ),
            Param(
                "T''qo",
                "Time constants",
                "s",
                "q-axis subtransient open-circuit time constant",
            ),
            Param(
                "H",
                "Mechanics",
                "s",
                "inertia constant (MW·s/MVA on the machine base)",
            ),
            Param("D", "Mechanics", "pu", "speed damping"),
            Param("Xd", "Reactances", "pu", "d-axis synchronous reactance"),
            Param("Xq", "Reactances", "pu", "q-axis synchronous reactance"),
            Param("X'd", "Reactances", "pu", "d-axis transient reactance"),
            Param(
                "X''d",
                "Reactances",
                "pu",
                "subtransient reactance (X''q = X''d)",
            ),
            Param("Xl", "Reactances", "pu", "stator leakage reactance"),
            Param(
                "S(1.0)",
                "Saturation",
                "",
                "saturation factor at 1.0 pu flux",
            ),
            Param(
                "S(1.2)",
                "Saturation",
                "",
                "saturation factor at 1.2 pu flux",
            ),
        ],
    ),
    (
        "GENSAE",
        &[
            Param(
                "T'do",
                "Time constants",
                "s",
                "d-axis transient open-circuit time constant",
            ),
            Param(
                "T''do",
                "Time constants",
                "s",
                "d-axis subtransient open-circuit time constant",
            ),
            Param(
                "T''qo",
                "Time constants",
                "s",
                "q-axis subtransient open-circuit time constant",
            ),
            Param(
                "H",
                "Mechanics",
                "s",
                "inertia constant (MW·s/MVA on the machine base)",
            ),
            Param("D", "Mechanics", "pu", "speed damping"),
            Param("Xd", "Reactances", "pu", "d-axis synchronous reactance"),
            Param("Xq", "Reactances", "pu", "q-axis synchronous reactance"),
            Param("X'd", "Reactances", "pu", "d-axis transient reactance"),
            Param(
                "X''d",
                "Reactances",
                "pu",
                "subtransient reactance (X''q = X''d)",
            ),
            Param("Xl", "Reactances", "pu", "stator leakage reactance"),
            Param(
                "S(1.0)",
                "Saturation",
                "",
                "saturation factor at 1.0 pu flux",
            ),
            Param(
                "S(1.2)",
                "Saturation",
                "",
                "saturation factor at 1.2 pu flux",
            ),
        ],
    ),
    (
        "SEXS",
        &[
            Param("TA/TB", "Regulator", "", "lead-lag ratio"),
            Param("TB", "Regulator", "s", "lead-lag lag time constant"),
            Param("K", "Regulator", "pu", "regulator gain"),
            Param("TE", "Exciter", "s", "exciter time constant"),
            Param("EMIN", "Limits", "pu", "minimum field voltage"),
            Param("EMAX", "Limits", "pu", "maximum field voltage"),
        ],
    ),
    (
        "ESAC1A",
        &[
            Param("TR", "Measurement", "s", "voltage transducer time constant"),
            Param("TB", "Regulator", "s", "lead-lag lag time constant"),
            Param("TC", "Regulator", "s", "lead-lag lead time constant"),
            Param("KA", "Regulator", "pu", "regulator gain"),
            Param("TA", "Regulator", "s", "regulator time constant"),
            Param("VAMAX", "Limits", "pu", "regulator output maximum"),
            Param("VAMIN", "Limits", "pu", "regulator output minimum"),
            Param("TE", "Exciter", "s", "exciter time constant"),
            Param("KF", "Feedback", "pu", "rate feedback gain"),
            Param("TF", "Feedback", "s", "rate feedback time constant"),
            Param("KC", "Exciter", "pu", "rectifier loading factor"),
            Param("KD", "Exciter", "pu", "demagnetizing factor"),
            Param("KE", "Exciter", "pu", "exciter constant"),
            Param("E1", "Saturation", "pu", "first saturation point"),
            Param("SE(E1)", "Saturation", "", "saturation at E1"),
            Param("E2", "Saturation", "pu", "second saturation point"),
            Param("SE(E2)", "Saturation", "", "saturation at E2"),
            Param("VRMAX", "Limits", "pu", "exciter input maximum"),
            Param("VRMIN", "Limits", "pu", "exciter input minimum"),
        ],
    ),
    (
        "EXST1",
        &[
            Param("TR", "Measurement", "s", "voltage transducer time constant"),
            Param("VIMAX", "Limits", "pu", "error signal maximum"),
            Param("VIMIN", "Limits", "pu", "error signal minimum"),
            Param("TC", "Regulator", "s", "lead-lag lead time constant"),
            Param("TB", "Regulator", "s", "lead-lag lag time constant"),
            Param("KA", "Regulator", "pu", "regulator gain"),
            Param("TA", "Regulator", "s", "regulator time constant"),
            Param("VRMAX", "Limits", "pu", "output maximum (times Vt)"),
            Param("VRMIN", "Limits", "pu", "output minimum (times Vt)"),
            Param("KC", "Limits", "pu", "rectifier loading factor"),
            Param("KF", "Feedback", "pu", "rate feedback gain"),
            Param("TF", "Feedback", "s", "rate feedback time constant"),
        ],
    ),
    (
        "TGOV1",
        &[
            Param("R", "Droop", "pu", "permanent droop"),
            Param("T1", "Valve", "s", "valve time constant"),
            Param("VMAX", "Valve", "pu", "valve maximum"),
            Param("VMIN", "Valve", "pu", "valve minimum"),
            Param("T2", "Turbine", "s", "turbine lead time constant"),
            Param("T3", "Turbine", "s", "turbine lag time constant"),
            Param("Dt", "Damping", "pu", "turbine damping"),
        ],
    ),
    (
        "GAST",
        &[
            Param("R", "Droop", "pu", "permanent droop"),
            Param("T1", "Valve", "s", "fuel valve time constant"),
            Param("T2", "Turbine", "s", "fuel system time constant"),
            Param("T3", "Load limit", "s", "load limit time constant"),
            Param("AT", "Load limit", "pu", "ambient temperature load limit"),
            Param("KT", "Load limit", "pu", "temperature limiter gain"),
            Param("VMAX", "Valve", "pu", "fuel valve maximum"),
            Param("VMIN", "Valve", "pu", "fuel valve minimum"),
            Param("Dturb", "Damping", "pu", "turbine damping"),
        ],
    ),
    (
        "HYGOV",
        &[
            Param("R", "Droop", "pu", "permanent droop"),
            Param("r", "Droop", "pu", "temporary droop"),
            Param("Tr", "Droop", "s", "governor time constant"),
            Param("Tf", "Gate", "s", "filter time constant"),
            Param("Tg", "Gate", "s", "servo time constant"),
            Param("VELM", "Gate", "pu/s", "gate velocity limit"),
            Param("GMAX", "Gate", "pu", "gate maximum"),
            Param("GMIN", "Gate", "pu", "gate minimum"),
            Param("TW", "Turbine", "s", "water time constant"),
            Param("At", "Turbine", "pu", "turbine gain"),
            Param("Dturb", "Damping", "pu", "turbine damping"),
            Param("qNL", "Turbine", "pu", "no-load flow"),
        ],
    ),
    (
        "IEEEST",
        &[
            Param(
                "IC",
                "Input",
                "",
                "input code (1 speed, 3 electrical power on MBASE)",
            ),
            Param("REMBUS", "Input", "", "remote bus (0: own terminal)"),
            Param("A1", "Filter", "s", "torsional filter coefficient"),
            Param("A2", "Filter", "s²", "torsional filter coefficient"),
            Param("A3", "Filter", "s", "torsional filter coefficient"),
            Param("A4", "Filter", "s²", "torsional filter coefficient"),
            Param("A5", "Filter", "s", "torsional filter coefficient"),
            Param("A6", "Filter", "s²", "torsional filter coefficient"),
            Param("T1", "Lead-lag", "s", "first lead time constant"),
            Param("T2", "Lead-lag", "s", "first lag time constant"),
            Param("T3", "Lead-lag", "s", "second lead time constant"),
            Param("T4", "Lead-lag", "s", "second lag time constant"),
            Param(
                "T5",
                "Gain and washout",
                "s",
                "washout numerator time constant",
            ),
            Param("T6", "Gain and washout", "s", "washout time constant"),
            Param("KS", "Gain and washout", "pu", "stabilizer gain"),
            Param("LSMAX", "Output limits", "pu", "output maximum"),
            Param("LSMIN", "Output limits", "pu", "output minimum"),
            Param(
                "VCU",
                "Output limits",
                "pu",
                "terminal voltage above which the output is cut",
            ),
            Param(
                "VCL",
                "Output limits",
                "pu",
                "terminal voltage below which the output is cut",
            ),
        ],
    ),
];

fn names(model: &str) -> Option<&'static [Param]> {
    let m = model.trim().to_ascii_uppercase();
    SCHEMA.iter().find(|(n, _)| *n == m).map(|(_, p)| *p)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DyrParameter {
    pub name: String,
    /// The token as written in the file.
    pub text: String,
    pub value: f64,
    /// Bucket of the parameter (reactances, time constants, limits, ...).
    pub group: String,
    /// Unit (`pu` on the machine base, `s`, ...).
    pub unit: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DyrRecordView {
    /// First line of the record in the file (1-based); edits name it.
    pub line: usize,
    pub model: String,
    /// `machine`, `exciter`, `governor` or `stabilizer`.
    pub role: String,
    pub parameters: Vec<DyrParameter>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DyrGeneratorView {
    pub bus: usize,
    pub machine_id: String,
    /// Machine base (MVA).
    pub mbase: f64,
    /// Load flow set points and limits (MW, MVAr, pu).
    pub pg: f64,
    pub pmax: f64,
    pub qmax: f64,
    pub qmin: f64,
    pub vg: f64,
    /// Linked records (machine, exciter, governor, stabilizer).
    pub records: Vec<DyrRecordView>,
    /// Inverter-based resource records of the unit (REGCA1, REECB1, ...),
    /// kept and not simulated in RMS.
    pub inverter: Vec<String>,
}

/// One parameter edit: record line, parameter index (0-based, PSS/E order),
/// new value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DyrEdit {
    pub line: usize,
    pub index: usize,
    pub value: f64,
}

fn record_at(dyr: &DynamicCase, line: usize) -> Option<&DynamicRecord> {
    dyr.records().iter().find(|r| r.line() == line)
}

/// Every in-service generator, its linked records as named parameters
/// (PSS/E order) and its inverter records; a static unit has none.
#[must_use]
pub fn dyr_parameters(network: &BalancedNetwork, dyr: &DynamicCase) -> Vec<DyrGeneratorView> {
    let linking = powerio::link_machines(network, dyr);
    let mut out = Vec::new();
    for m in &linking.machines {
        let g = &network.generators()[m.generator];
        let mut records = Vec::new();
        for (role, line) in [
            ("machine", m.machine.as_ref().map(|l| l.line)),
            ("exciter", m.exciter.as_ref().map(|l| l.line)),
            ("governor", m.governor.as_ref().map(|l| l.line)),
            ("stabilizer", m.stabilizer.as_ref().map(|l| l.line)),
        ] {
            let Some(line) = line else { continue };
            let Some(rec) = record_at(dyr, line) else {
                continue;
            };
            let model = rec.model_name().trim().to_ascii_uppercase();
            let Some(pnames) = names(&model) else {
                continue;
            };
            let parameters = rec
                .parameters()
                .iter()
                .zip(pnames.iter())
                .map(|(t, p)| DyrParameter {
                    name: p.0.to_string(),
                    text: t.trim().to_string(),
                    value: t.trim().parse().unwrap_or(f64::NAN),
                    group: p.1.to_string(),
                    unit: p.2.to_string(),
                    description: p.3.to_string(),
                })
                .collect();
            records.push(DyrRecordView {
                line,
                model,
                role: role.to_string(),
                parameters,
            });
        }
        let inverter = dyr
            .records()
            .iter()
            .filter(|r| {
                r.bus_id().is_some_and(|b| b as usize == m.bus)
                    && r.device_id().unwrap_or("1").trim() == m.machine_id
                    && powerio::typed_model(r).is_err()
            })
            .map(|r| r.model_name().trim().to_ascii_uppercase())
            .collect();
        out.push(DyrGeneratorView {
            bus: m.bus,
            machine_id: m.machine_id.clone(),
            mbase: m.mbase,
            pg: g.pg,
            pmax: g.pmax,
            qmax: g.qmax,
            qmin: g.qmin,
            vg: g.vg,
            records,
            inverter,
        });
    }
    out
}

/// Byte spans of a record's fields: separated by whitespace or commas, a
/// quoted field is one token, `/` ends the record.
fn token_spans(raw: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start: Option<usize> = None;
    let mut quoted = false;
    for (i, c) in raw.char_indices() {
        if quoted {
            if c == '\'' {
                quoted = false;
            }
            continue;
        }
        match c {
            '\'' => {
                start.get_or_insert(i);
                quoted = true;
            }
            '/' => {
                if let Some(s) = start.take() {
                    spans.push((s, i));
                }
                return spans;
            }
            c if c.is_whitespace() || c == ',' => {
                if let Some(s) = start.take() {
                    spans.push((s, i));
                }
            }
            _ => {
                start.get_or_insert(i);
            }
        }
    }
    if let Some(s) = start {
        spans.push((s, raw.len()));
    }
    spans
}

/// Byte offset of the start of 1-based `line` in `text`.
fn line_offset(text: &str, line: usize) -> Option<usize> {
    if line == 0 {
        return None;
    }
    if line == 1 {
        return Some(0);
    }
    text.match_indices('\n').nth(line - 2).map(|(i, _)| i + 1)
}

/// Shortest decimal text that reads back as `v`.
fn number_text(v: f64) -> String {
    let s = format!("{v}");
    if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("NaN") {
        s
    } else {
        format!("{s}.0")
    }
}

/// `text` with the edited parameter tokens replaced.
///
/// # Errors
/// When a line names no record, the index is out of range, the value is not
/// finite, or the record's text cannot be located; the result is read back
/// and must parse to the edited values.
pub fn dyr_edit(text: &str, edits: &[DyrEdit]) -> Result<String, String> {
    let mut out = text.to_string();
    for e in edits {
        if !e.value.is_finite() {
            return Err(format!("line {}: value is not finite", e.line));
        }
        let dyr = powerio::parse_dyr(&out).map_err(|err| format!("DYR: {err}"))?;
        let rec = record_at(&dyr, e.line)
            .ok_or_else(|| format!("no record starts on line {}", e.line))?;
        let n = rec.parameters().len();
        if e.index >= n {
            return Err(format!(
                "line {}: {} has {n} parameters, no index {}",
                e.line,
                rec.model_name(),
                e.index
            ));
        }
        let from =
            line_offset(&out, e.line).ok_or_else(|| format!("line {} is past the end", e.line))?;
        let raw = rec.raw_record();
        let at = out[from..]
            .find(raw)
            .map(|k| from + k)
            .ok_or_else(|| format!("line {}: record text not found", e.line))?;
        let spans = token_spans(raw);
        if spans.len() < n {
            return Err(format!("line {}: record fields not found", e.line));
        }
        let (s, t) = spans[spans.len() - n + e.index];
        if raw[s..t].trim() != rec.parameters()[e.index].trim() {
            return Err(format!(
                "line {}: field {:?} does not match parameter {:?}",
                e.line,
                &raw[s..t],
                rec.parameters()[e.index]
            ));
        }
        out.replace_range(at + s..at + t, &number_text(e.value));
    }
    // Read back: every edited parameter holds its last value.
    let dyr = powerio::parse_dyr(&out).map_err(|err| format!("edited DYR: {err}"))?;
    let mut last = std::collections::BTreeMap::new();
    for e in edits {
        last.insert((e.line, e.index), e.value);
    }
    for ((line, index), value) in last {
        let rec = record_at(&dyr, line).ok_or_else(|| format!("edited DYR lost line {line}"))?;
        let got: f64 = rec.parameters()[index]
            .trim()
            .parse()
            .map_err(|_| format!("line {line}: edited value unreadable"))?;
        if got != value {
            return Err(format!("line {line}: edited value reads back as {got}"));
        }
    }
    Ok(out)
}

/// [`dyr_parameters`] for a stored PowerIO module and DYR text, as JSON.
///
/// # Errors
/// When the inputs do not parse.
pub fn dyr_parameters_json(module_json: &str, dyr_text: &str) -> Result<String, String> {
    let module = crate::ir::balanced_module(crate::ir::deserialize_module(module_json)?)?;
    let dyr = powerio::parse_dyr(dyr_text).map_err(|e| format!("DYR: {e}"))?;
    serde_json::to_string(&dyr_parameters(module.value(), &dyr)).map_err(|e| e.to_string())
}

/// [`dyr_edit`] with the edits as JSON (`[{"line", "index", "value"}]`).
///
/// # Errors
/// As [`dyr_edit`], or when the edits do not parse.
pub fn dyr_edit_json(dyr_text: &str, edits_json: &str) -> Result<String, String> {
    let edits: Vec<DyrEdit> = serde_json::from_str(edits_json).map_err(|e| e.to_string())?;
    dyr_edit(dyr_text, &edits)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "\n  1 'GENROU' 1   8.0 0.03 0.4 0.05 6.5 0.0 1.8 1.7 0.3 0.55 0.25 0.2\n     0.0 0.0 /\n 1 'SEXS' 1 0.1 10.0 100.0, 0.1 0.0 3.0 /\n";

    #[test]
    fn schema_matches_powerio_arity() {
        for (model, arity) in powerio::SUPPORTED_MODELS {
            assert_eq!(names(model).map(<[_]>::len), Some(*arity), "{model}");
        }
    }

    #[test]
    fn edit_replaces_one_token_only() {
        let dyr = powerio::parse_dyr(TEXT).unwrap();
        let sexs = dyr
            .records()
            .iter()
            .find(|r| r.model_name() == "SEXS")
            .unwrap();
        let genrou = dyr
            .records()
            .iter()
            .find(|r| r.model_name() == "GENROU")
            .unwrap();
        let out = dyr_edit(
            TEXT,
            &[
                DyrEdit {
                    line: sexs.line(),
                    index: 2,
                    value: 200.0,
                },
                DyrEdit {
                    line: genrou.line(),
                    index: 13,
                    value: 0.125,
                },
            ],
        )
        .unwrap();
        assert_eq!(
            out,
            "\n  1 'GENROU' 1   8.0 0.03 0.4 0.05 6.5 0.0 1.8 1.7 0.3 0.55 0.25 0.2\n     0.0 0.125 /\n 1 'SEXS' 1 0.1 10.0 200.0, 0.1 0.0 3.0 /\n"
        );
    }

    /// On the example cases: every linked record, one parameter changed,
    /// every other record and parameter as before.
    #[test]
    fn edit_every_linked_record_of_the_examples() {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
        for name in ["rms_examples/wecc240"] {
            let raw = powerio::Source::open(base.join(name).join("case.raw")).unwrap();
            let module = powerio::parse(raw).unwrap();
            let powerio::PioValue::BalancedNetwork(net) = module.value() else {
                panic!("not a balanced network")
            };
            let text = std::fs::read_to_string(base.join(name).join("case.dyr")).unwrap();
            let dyr = powerio::parse_dyr(&text).unwrap();
            let views = dyr_parameters(net, &dyr);
            assert!(!views.is_empty());
            let mut edited = 0;
            for g in &views {
                for r in &g.records {
                    let k = r.parameters.len() - 1;
                    let value = r.parameters[k].value + 0.25;
                    let out = dyr_edit(
                        &text,
                        &[DyrEdit {
                            line: r.line,
                            index: k,
                            value,
                        }],
                    )
                    .unwrap();
                    let after = powerio::parse_dyr(&out).unwrap();
                    assert_eq!(after.records().len(), dyr.records().len());
                    for (a, b) in dyr.records().iter().zip(after.records()) {
                        if a.line() == r.line {
                            assert_eq!(b.parameters()[k].parse::<f64>().unwrap(), value);
                            assert_eq!(a.parameters()[..k], b.parameters()[..k]);
                        } else {
                            assert_eq!(a.raw_record(), b.raw_record());
                        }
                    }
                    edited += 1;
                }
            }
            println!(
                "{name}: {} generators, {edited} records edited",
                views.len()
            );
        }
    }

    #[test]
    fn edit_rejects_bad_requests() {
        assert!(dyr_edit(
            TEXT,
            &[DyrEdit {
                line: 1,
                index: 0,
                value: 1.0
            }]
        )
        .is_err());
        let line = powerio::parse_dyr(TEXT).unwrap().records()[1].line();
        assert!(dyr_edit(
            TEXT,
            &[DyrEdit {
                line,
                index: 6,
                value: 1.0
            }]
        )
        .is_err());
        assert!(dyr_edit(
            TEXT,
            &[DyrEdit {
                line,
                index: 0,
                value: f64::NAN
            }]
        )
        .is_err());
    }
}
