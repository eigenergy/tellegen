//! Parse `$PGLIB_OPF_PATH/BASELINE.md` into a reference map. The file holds three
//! markdown tables — Typical (TYP), Congested (API), Small-Angle (SAD) — each with
//! the columns `DC ($/h)`, `AC ($/h)`, `QC Gap (%)`, `SOC Gap (%)`, and per-relaxation
//! times. We keep the four objective/gap columns, keyed by `(base case name, variant)`.
//!
//! The published values come from PowerModels.jl + IPOPT (see the file header); they
//! are the independent OPF baseline tellegen is validated against. Some DC cells read
//! `inf.` (the DC relaxation was infeasible, common in the SAD set) — those map to
//! `None`.

use std::collections::HashMap;
use std::path::Path;

use crate::corpus::Variant;

/// One BASELINE.md row: node/edge counts and the four objective/gap figures. A cell
/// that the source left non-numeric (`inf.`, blank) is `None`.
#[derive(Clone, Debug)]
pub struct BaselineRow {
    pub nodes: usize,
    /// DC OPF objective, $/h.
    pub dc: Option<f64>,
    /// AC OPF objective, $/h (the relaxation lower bound's reference).
    pub ac: Option<f64>,
    /// QC relaxation optimality gap, percent.
    pub qc_gap: Option<f64>,
    /// SOC (Jabr) relaxation optimality gap, percent — the column tellegen's SOCWR
    /// relaxation is compared against.
    pub soc_gap: Option<f64>,
}

pub type BaselineMap = HashMap<(String, Variant), BaselineRow>;

/// Parse a numeric cell, mapping `inf.`/blank/`—` to `None`.
fn parse_num(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() || t == "inf." || t == "—" || t == "-" || t == "N/A" {
        return None;
    }
    t.parse().ok()
}

/// Parse BASELINE.md into `(base case name, variant) → row`. A missing file yields an
/// empty map; the OPF-correctness metric then simply has no reference to compare.
pub fn parse(path: &Path) -> BaselineMap {
    let mut map = BaselineMap::new();
    let Ok(text) = std::fs::read_to_string(path) else {
        return map;
    };
    let mut current: Option<Variant> = None;
    for line in text.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("##") {
            current = if rest.contains("(TYP)") {
                Some(Variant::Typ)
            } else if rest.contains("(API)") {
                Some(Variant::Api)
            } else if rest.contains("(SAD)") {
                Some(Variant::Sad)
            } else {
                None
            };
            continue;
        }
        let Some(variant) = current else { continue };
        if !l.starts_with("| pglib_opf_case") {
            continue;
        }
        // `| name | nodes | edges | DC | AC | QC | SOC | ...times... |`
        // split('|') yields a leading "" before the first pipe, so column k is cells[k].
        let cells: Vec<&str> = l.split('|').map(str::trim).collect();
        if cells.len() < 8 {
            continue;
        }
        // Reuse the corpus key derivation so the BASELINE row and the case it joins to
        // normalize identically; a private `trim_end_matches` chain here would over-trim a
        // base name that itself ends in `__api`/`__sad` and silently fail to join.
        let base = crate::corpus::base_name(cells[1]);
        let row = BaselineRow {
            nodes: cells[2].parse().unwrap_or(0),
            dc: parse_num(cells[4]),
            ac: parse_num(cells[5]),
            qc_gap: parse_num(cells[6]),
            soc_gap: parse_num(cells[7]),
        };
        map.insert((base, variant), row);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_corpus_baseline_when_present() {
        let path = crate::corpus::corpus_root().join("BASELINE.md");
        if !path.exists() {
            eprintln!("skipping baseline parse test: {} absent", path.display());
            return;
        }
        let map = parse(&path);
        // The three variants of case3_lmbd are present with finite objectives.
        let typ = map
            .get(&("pglib_opf_case3_lmbd".to_string(), Variant::Typ))
            .expect("case3 typ row");
        assert_eq!(typ.nodes, 3);
        assert!(
            (typ.dc.unwrap() - 5695.9).abs() < 1.0,
            "case3 DC {:?}",
            typ.dc
        );
        assert!(typ.soc_gap.unwrap() > 0.0);
        // The SAD case5 DC cell is `inf.` in the source → None.
        let sad5 = map.get(&("pglib_opf_case5_pjm".to_string(), Variant::Sad));
        if let Some(r) = sad5 {
            assert!(
                r.dc.is_none(),
                "case5 sad DC should be inf./None: {:?}",
                r.dc
            );
            assert!(r.ac.is_some());
        }
    }
}
