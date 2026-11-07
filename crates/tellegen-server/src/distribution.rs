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

/// Hosted overrides use the same setting keys and validation as other callers.
pub type PfOptions = tellegen::settings::Options;

impl Metadata {
    fn validate(&self) -> Result<(), String> {
        tellegen::settings::resolve(
            &tellegen::settings::SettingsRequest::new(tellegen::settings::Calculation::McAcPf)
                .with_baseline(self.pf_options.clone()),
        )?;
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hosted_overrides_accept_all_mc_fields_and_share_the_common_validator() {
        let mut metadata:Metadata=serde_json::from_value(serde_json::json!({"pf_options":{"damping":0.8,"relative_kcl_tolerance":0,"v_low_pu":0.4,"v_min_pu":0.8,"v_max_pu":1.2}})).unwrap();
        metadata.validate().unwrap();
        metadata
            .pf_options
            .insert("v_min_pu".into(), serde_json::json!(0.3));
        assert!(metadata.validate().is_err());
        metadata.pf_options.remove("v_min_pu");
        metadata
            .pf_options
            .insert("typo".into(), serde_json::json!(1));
        assert!(metadata
            .validate()
            .unwrap_err()
            .contains("unsupported setting"));
    }
}
