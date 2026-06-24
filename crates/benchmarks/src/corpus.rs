//! Enumerate the PGLib-OPF corpus without vendoring it: walk the three variant
//! directories under `$PGLIB_OPF_PATH` and return one [`CaseFile`] per `.m` file.
//!
//! Bus counts come from the filename (`pglib_opf_case<N>_*`), so `--max-bus` can
//! skip the giant cases before any file is read.

use std::path::{Path, PathBuf};

use serde::Serialize;
use walkdir::WalkDir;

/// The three PGLib operating-condition sets: Typical, Congested (API), Small-Angle
/// Difference (SAD).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub enum Variant {
    Typ,
    Api,
    Sad,
}

impl Variant {
    /// Lowercase tag for filenames, CSV cells, and CLI flags.
    pub fn tag(self) -> &'static str {
        match self {
            Variant::Typ => "typ",
            Variant::Api => "api",
            Variant::Sad => "sad",
        }
    }

    pub const ALL: [Variant; 3] = [Variant::Typ, Variant::Api, Variant::Sad];

    /// Subdirectory under the corpus root. The typical set lives at the root; the
    /// congested and small-angle sets live in `api/` and `sad/`.
    fn subdir(self) -> &'static str {
        match self {
            Variant::Typ => "",
            Variant::Api => "api",
            Variant::Sad => "sad",
        }
    }

    /// Filename suffix before `.m`. Typical files carry none.
    fn suffix(self) -> &'static str {
        match self {
            Variant::Typ => "",
            Variant::Api => "__api",
            Variant::Sad => "__sad",
        }
    }

    /// Parse a CLI variant-set token (`typ`/`api`/`sad`/`all`) into the variants it
    /// selects.
    pub fn parse_set(s: &str) -> Option<Vec<Variant>> {
        match s {
            "all" => Some(Variant::ALL.to_vec()),
            "typ" => Some(vec![Variant::Typ]),
            "api" => Some(vec![Variant::Api]),
            "sad" => Some(vec![Variant::Sad]),
            _ => None,
        }
    }
}

/// One corpus file: the base case name (suffix stripped, so it keys the BASELINE.md
/// map across variants), the variant, the path, and the bus count from the filename.
#[derive(Clone, Debug)]
pub struct CaseFile {
    pub case: String,
    pub variant: Variant,
    pub path: PathBuf,
    pub buses: usize,
}

/// Bus count from a PGLib filename: the digit run immediately after `case`.
/// `pglib_opf_case2383wp_k` → 2383, `pglib_opf_case30000_goc` → 30000.
pub fn bus_count_from_name(name: &str) -> Option<usize> {
    let i = name.find("case")?;
    let digits: String = name[i + 4..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

/// Base case name: strip the variant suffix so `pglib_opf_case3_lmbd__api`
/// and `pglib_opf_case3_lmbd` share one key.
pub(crate) fn base_name(stem: &str) -> String {
    // Strip a single variant suffix. `trim_end_matches` would peel every repeated trailing
    // copy and over-trim a base name that itself ends in the token, so anchor to one suffix.
    stem.strip_suffix("__api")
        .or_else(|| stem.strip_suffix("__sad"))
        .unwrap_or(stem)
        .to_string()
}

/// Resolve the corpus root: `$PGLIB_OPF_PATH`, else `~/Datasets/pglib-opf`.
pub fn corpus_root() -> PathBuf {
    if let Ok(p) = std::env::var("PGLIB_OPF_PATH") {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join("Datasets/pglib-opf")
}

/// Enumerate the requested variants under `root`, sorted by bus count then name, so
/// the runner takes the small cases first and the giants last. Returns an empty list
/// when the root is absent — the caller reports the skip.
pub fn enumerate(root: &Path, variants: &[Variant]) -> Vec<CaseFile> {
    let mut out = Vec::new();
    for &v in variants {
        let dir = if v.subdir().is_empty() {
            root.to_path_buf()
        } else {
            root.join(v.subdir())
        };
        if !dir.is_dir() {
            continue;
        }
        // max_depth(1) keeps the typical walk from descending into api/ and sad/.
        for entry in WalkDir::new(&dir)
            .max_depth(1)
            .into_iter()
            .filter_map(Result::ok)
        {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) != Some("m") {
                continue;
            }
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            // A typical file must carry no variant suffix; api/sad must carry theirs.
            let suf = v.suffix();
            let well_formed = if suf.is_empty() {
                !stem.ends_with("__api") && !stem.ends_with("__sad")
            } else {
                stem.ends_with(suf)
            };
            if !well_formed {
                continue;
            }
            out.push(CaseFile {
                case: base_name(stem),
                variant: v,
                path: p.to_path_buf(),
                // A name whose `case<N>` digit run does not parse has an unknown size. Sort it
                // last (not first) and let the size guards skip the heavy stages, rather than
                // defaulting to 0 — which would float it ahead of the small cases and slip it
                // past every `cf.buses <= max_*` guard into a full run.
                buses: bus_count_from_name(stem).unwrap_or(usize::MAX),
            });
        }
    }
    out.sort_by(|a, b| {
        a.buses
            .cmp(&b.buses)
            .then_with(|| a.case.cmp(&b.case))
            .then_with(|| a.variant.tag().cmp(b.variant.tag()))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_count_parses_pglib_names() {
        assert_eq!(bus_count_from_name("pglib_opf_case3_lmbd"), Some(3));
        assert_eq!(bus_count_from_name("pglib_opf_case2383wp_k"), Some(2383));
        assert_eq!(
            bus_count_from_name("pglib_opf_case30000_goc__api"),
            Some(30000)
        );
        assert_eq!(
            bus_count_from_name("pglib_opf_case78484_epigrids__sad"),
            Some(78484)
        );
    }

    #[test]
    fn base_name_strips_suffix() {
        assert_eq!(base_name("pglib_opf_case5_pjm__api"), "pglib_opf_case5_pjm");
        assert_eq!(base_name("pglib_opf_case5_pjm__sad"), "pglib_opf_case5_pjm");
        assert_eq!(base_name("pglib_opf_case5_pjm"), "pglib_opf_case5_pjm");
    }
}
