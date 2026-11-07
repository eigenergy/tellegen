//! Operator-staged portable distribution modules. The browser owns their
//! presentation and AC power flow; the server retains the original typed module.

use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path},
};

use powerio::PioValue;
use serde::{Deserialize, Serialize};

use crate::{CaseModel, CaseSummary, UnavailableCase};

pub(crate) struct DistributionCase {
    pub summary: CaseSummary,
    pub module_json: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Spec {
    id: String,
    name: String,
    file: String,
    #[serde(default)]
    metadata: Metadata,
}

/// Public presentation and reproducible operating-point defaults for a hosted case.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub description: Option<String>,
    pub source_url: Option<String>,
    pub related_case_id: Option<String>,
    #[serde(default)]
    pub pf_options: PfOptions,
}

/// Supported hosted overrides; omitted values retain engine defaults.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PfOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub absolute_kcl_tolerance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_iterations: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voltage_envelope: Option<bool>,
}

impl Metadata {
    fn validate(&self) -> Result<(), String> {
        let options = &self.pf_options;
        if [options.tolerance, options.absolute_kcl_tolerance]
            .into_iter()
            .flatten()
            .any(|v| !v.is_finite() || v <= 0.0)
            || options.max_iterations == Some(0)
        {
            return Err("hosted PF tolerances and iteration count must be positive".into());
        }
        if let Some(url) = &self.source_url {
            let uri: axum::http::Uri = url.parse().map_err(|_| "invalid source URL")?;
            if uri.scheme_str() != Some("https") || uri.host().is_none() {
                return Err("source URL must be an absolute HTTPS URL".into());
            }
        }
        if let Some(id) = &self.related_case_id {
            if !crate::CASE_SPECS.iter().any(|spec| spec.id == id) {
                return Err(format!("unknown related transmission case: {id}"));
            }
        }
        Ok(())
    }
}

pub(crate) fn load(
    data: &Path,
) -> Result<(BTreeMap<String, DistributionCase>, Vec<UnavailableCase>), String> {
    let manifest = data.join("distribution-cases.json");
    let text = match fs::read_to_string(&manifest) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((BTreeMap::new(), vec![])),
        Err(e) => return Err(format!("{}: {e}", manifest.display())),
    };
    let specs: Vec<Spec> =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let mut ids = std::collections::HashSet::new();
    for spec in &specs {
        if spec.id.is_empty()
            || spec.id.starts_with("local-")
            || spec.id.starts_with("dist-")
            || !spec
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || spec.name.trim().is_empty()
        {
            return Err("distribution cases require a nonempty name and an ASCII alphanumeric, '-' or '_' ID without the reserved local-/dist- prefixes".into());
        }
        if !ids.insert(&spec.id) || crate::CASE_SPECS.iter().any(|s| s.id == spec.id) {
            return Err(format!(
                "duplicate or reserved distribution case ID: {}",
                spec.id
            ));
        }
    }
    let mut cases = BTreeMap::new();
    let mut unavailable = vec![];
    for spec in specs {
        match load_case(data, &spec) {
            Ok(case) => {
                cases.insert(spec.id, case);
            }
            Err(reason) => {
                tracing::error!(case = spec.id, %reason, "distribution case unavailable");
                unavailable.push(UnavailableCase {
                    id: spec.id,
                    name: spec.name,
                    model: CaseModel::Multiconductor,
                    reason,
                });
            }
        }
    }
    Ok((cases, unavailable))
}

fn load_case(data: &Path, spec: &Spec) -> Result<DistributionCase, String> {
    spec.metadata.validate()?;
    let relative = Path::new(&spec.file);
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
        || spec.file.is_empty()
    {
        return Err("distribution file must be a relative path within the data directory".into());
    }
    let root = data.canonicalize().map_err(|e| e.to_string())?;
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|e| format!("{}: {e}", spec.file))?;
    if !path.starts_with(&root) {
        return Err("distribution file must stay within the data directory".into());
    }
    let module_json = fs::read_to_string(path).map_err(|e| format!("{}: {e}", spec.file))?;
    let module = tellegen::ir::deserialize_module(&module_json)?;
    let net = match module.value() {
        PioValue::MulticonductorNetwork(net) => net,
        PioValue::McAcPfInstance(instance) => instance.network(),
        PioValue::McAcOpfInstance(instance) => instance.network(),
        PioValue::McAcPfSolution(solution) => solution.network(),
        PioValue::McAcOpfSolution(solution) => solution.network(),
        other => {
            return Err(format!(
                "expected a multiconductor module, got {}",
                other.type_name()
            ))
        }
    };
    let graph = net.to_graph();
    Ok(DistributionCase {
        summary: CaseSummary {
            id: spec.id.clone(),
            name: spec.name.clone(),
            distribution: Some(spec.metadata.clone()),
            model: CaseModel::Multiconductor,
            unavailable_reason: None,
            n_bus: graph.buses.len(),
            n_branch: graph.edges.len(),
            n_analysis_bus: graph.buses.len(),
            n_analysis_branch: graph.edges.len(),
            n_gen: net.generators().len() + net.ibrs().len(),
        },
        module_json,
    })
}
