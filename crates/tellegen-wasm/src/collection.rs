//! Browser-owned collection sessions. PowerIO owns acquisition and selection;
//! each imported module is independent of this session and its source bytes.

use std::sync::Arc;

use powerio::collection::{CollectionOperation, SourceBundleScope};
use powerio::{
    ArtifactPath, Collection, CollectionLimits, CollectionSelection, ParseOptions, PioValue,
};
#[cfg(test)]
use serde_json::json;
use serde_json::Value;
use wasm_bindgen::prelude::*;

use crate::{dist, jserr};

#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub(crate) struct Selection {
    root: Option<String>,
    primary: Option<String>,
    members: Option<Vec<String>>,
    format: Option<String>,
    operation: Operation,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "kebab-case")]
enum Operation {
    Parse,
    Deserialize,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "kebab-case")]
enum Status {
    Selectable,
    NeedsOptions,
    NeedsFormat,
    ProbeLimit,
    UnsupportedBuild,
    Incomplete,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
struct Candidate {
    selection: Selection,
    status: Status,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
struct Entry {
    path: String,
    bytes: u64,
}
#[derive(serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
struct Limits {
    files: usize,
    expanded_bytes: usize,
}
#[derive(serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub(crate) struct Inspection {
    name: String,
    entries: Vec<Entry>,
    candidates: Vec<Candidate>,
    original_zip: bool,
    limits: Limits,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
struct Origin {
    collection: String,
    containers: Vec<String>,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub(crate) struct SelectionOrigin {
    origin: Origin,
    selection: Selection,
}
#[derive(serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
enum Kind {
    Balanced,
    Multiconductor,
}
#[derive(serde::Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Imported {
    Balanced {
        #[cfg_attr(
            feature = "schema",
            schemars(
                with = "crate::outputs::WithModuleText<crate::outputs::BalancedIngest<'static>>"
            )
        )]
        payload: Value,
        origin: SelectionOrigin,
        dependencies: Vec<String>,
        incomplete: bool,
    },
    Multiconductor {
        #[cfg_attr(
            feature = "schema",
            schemars(with = "crate::outputs::WithModuleText<crate::outputs::DistributionIngest>")
        )]
        payload: Value,
        origin: SelectionOrigin,
        dependencies: Vec<String>,
        incomplete: bool,
    },
}

// Upstream collection records are serializable but do not derive JsonSchema.
// Convert at this boundary so the records used for output also define validation.
fn record<T: serde::de::DeserializeOwned>(value: impl serde::Serialize) -> Result<T, String> {
    serde_json::from_value(serde_json::to_value(value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

const MAX_BYTES: usize = 64 << 20;
const MAX_FILES: usize = 4096;
const MAX_SELECTION_BYTES: usize = 1 << 20;

fn browser_limits() -> CollectionLimits {
    // PowerIO conservatively charges the permitted project tree per parse.
    // Allow a full 32-case batch, acquisition and inspection without confusing
    // cumulative CPU/input work with the independent 64 MiB residency bound.
    // `CollectionLimits` is non-exhaustive, so start from PowerIO's defaults.
    let mut limits = CollectionLimits::default();
    limits.work_bytes = 36 * MAX_BYTES as u64;
    limits
}

/// A bounded ZIP or named file tree, held once until explicitly freed.
/// Neither discovery nor selection permits filesystem or network access.
#[wasm_bindgen]
pub struct CaseCollection(Collection);

#[wasm_bindgen]
impl CaseCollection {
    /// Acquire and verify a ZIP. Nested archives remain explicit inventory
    /// entries; they are not flattened or searched for missing dependencies.
    pub fn from_zip(name: &str, bytes: &[u8]) -> Result<CaseCollection, JsValue> {
        crate::install_panic_hook();
        Self::zip(name, bytes).map_err(jserr)
    }

    /// Acquire a tree in one bounded binary buffer, with N names and N+1
    /// offsets. This avoids base64 and one transfer per selected master.
    pub fn from_files(
        name: &str,
        names_json: &str,
        offsets: &[u32],
        bytes: &[u8],
    ) -> Result<CaseCollection, JsValue> {
        crate::install_panic_hook();
        Self::files(name, names_json, offsets, bytes).map_err(jserr)
    }

    /// Inventory and bounded candidate suggestions, without electrical parsing.
    pub fn inspect(&self) -> Result<String, JsValue> {
        self.inspect_value().map(|v| v.to_string()).map_err(jserr)
    }

    /// Explicitly inspect a child archive in its own bounded namespace.
    pub fn open_nested(&self, member: &str) -> Result<CaseCollection, JsValue> {
        self.0.open_nested(member).map(Self).map_err(jserr)
    }

    /// Parse one independent selection. The resulting IR and display payload
    /// own their data and remain usable after this collection is freed/cancelled.
    pub fn ingest(&self, selection_json: &str) -> Result<String, JsValue> {
        self.ingest_value(selection_json)
            .map(|v| v.to_string())
            .map_err(jserr)
    }

    /// Exact original container bytes; a file-tree session has no original ZIP.
    pub fn original_zip(&self) -> Result<Vec<u8>, JsValue> {
        self.0
            .original_zip()
            .map(|bytes| bytes.to_vec())
            .ok_or_else(|| jserr("this collection came from files, not a ZIP"))
    }

    /// Build a new ZIP with an explicit export scope. `project` includes all
    /// companions in the permitted root; `selected` includes only the primary
    /// or chosen document group and does not promise a self-contained project.
    pub fn source_zip(&self, selection_json: &str, scope: &str) -> Result<Vec<u8>, JsValue> {
        let selection = selection(selection_json).map_err(jserr)?;
        let scope = match scope {
            "selected" => SourceBundleScope::SelectedMembers,
            "project" => SourceBundleScope::Project,
            _ => return Err(jserr("choose selected or project source export scope")),
        };
        self.0
            .source_bundle(&selection, scope)
            .and_then(|bundle| bundle.deterministic_zip(CollectionLimits::default().archive))
            .map_err(jserr)
    }
}

impl CaseCollection {
    fn zip(name: &str, bytes: &[u8]) -> Result<Self, String> {
        check_name(name)?;
        if bytes.len() > MAX_BYTES {
            return Err("ZIP exceeds the 64 MiB compressed input limit".into());
        }
        Collection::from_zip(name, Arc::from(bytes), browser_limits())
            .map(Self)
            .map_err(|e| e.to_string())
    }

    fn files(name: &str, names_json: &str, offsets: &[u32], bytes: &[u8]) -> Result<Self, String> {
        check_name(name)?;
        if names_json.len() > MAX_SELECTION_BYTES || bytes.len() > MAX_BYTES {
            return Err("file collection exceeds its metadata or 64 MiB byte limit".into());
        }
        let names: Vec<String> = serde_json::from_str(names_json).map_err(|e| e.to_string())?;
        if names.len() > MAX_FILES
            || offsets.len() != names.len() + 1
            || offsets.first() != Some(&0)
            || offsets.last().copied().map(|n| n as usize) != Some(bytes.len())
            || offsets.windows(2).any(|w| w[0] > w[1])
        {
            return Err("invalid file collection offsets or entry count".into());
        }
        let files = names
            .iter()
            .zip(offsets.windows(2))
            .map(|(name, span)| {
                Ok((
                    ArtifactPath::new(name).map_err(|e| e.to_string())?,
                    Arc::from(&bytes[span[0] as usize..span[1] as usize]),
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        Collection::from_files(name, files, browser_limits())
            .map(Self)
            .map_err(|e| e.to_string())
    }

    fn inspect_value(&self) -> Result<Value, String> {
        serde_json::to_value(Inspection {
            name: self.0.name().to_owned(),
            entries: record(self.0.entries())?,
            candidates: record(self.0.inspect().map_err(|e| e.to_string())?)?,
            original_zip: self.0.original_zip().is_some(),
            limits: Limits {
                files: MAX_FILES,
                expanded_bytes: MAX_BYTES,
            },
        })
        .map_err(|e| e.to_string())
    }

    fn ingest_value(&self, selection_json: &str) -> Result<Value, String> {
        let selection = selection(selection_json)?;
        let result = self
            .0
            .parse(&selection, &ParseOptions::default())
            .map_err(|e| e.to_string())?;
        let mut module = result.module;
        // Origin is metadata, never an electrical property or a claim that IR
        // embeds the original source container.
        let origin =
            serde_json::to_value(self.0.selection_origin(&selection)).map_err(|e| e.to_string())?;
        module
            .insert_extension("tellegen.collection_origin", origin.clone())
            .map_err(|e| e.to_string())?;
        let parsed_source_is = |formats: &[&str]| {
            selection.operation == CollectionOperation::Parse
                && module.sources().iter().any(|source| {
                    source
                        .format()
                        .is_some_and(|format| formats.contains(&format.as_str()))
                })
        };
        let is_aux = parsed_source_is(&["aux", "powerworld"]);
        if parsed_source_is(&["bmopf-json", "bmopf"]) {
            let source = self.0.source(&selection).map_err(|e| e.to_string())?;
            let buffer = source.primary_buffer().map_err(|e| e.to_string())?;
            let text = std::str::from_utf8(buffer.bytes()).map_err(|e| e.to_string())?;
            #[cfg(feature = "mc-pf")]
            let reason = tellegen::validate_bmopf_json(text).err();
            #[cfg(not(feature = "mc-pf"))]
            let reason = {
                let _ = text;
                Some("MC PF support is disabled in this build".to_owned())
            };
            if let Some(reason) = reason {
                module = module
                    .with_diagnostic(powerio::Diagnostic::new(
                        powerio::DiagnosticCode::new("PARTNER.TELLEGEN.MC_PF_UNSUPPORTED")
                            .map_err(|e| e.to_string())?,
                        powerio::DiagnosticSeverity::Warning,
                        reason,
                    ))
                    .map_err(|e| e.to_string())?;
            }
        }
        let (kind, payload) = if dist::is_viewable_module_value(module.value()) {
            // The typed snapshot capability gate applies equally to raw files
            // and collection selections; ZIP admission cannot bypass it.
            (
                Kind::Multiconductor,
                dist::ingest_dist_module_value(module)?,
            )
        } else if crate::balanced_study_network(module.value()).is_some() {
            // AUX location tables live in retained source, outside the typed
            // electrical value, and must survive selected-source ingestion.
            if let PioValue::BalancedNetwork(network) = module.value_mut() {
                if is_aux {
                    let source = self.0.source(&selection).map_err(|e| e.to_string())?;
                    let buffer = source.primary_buffer().map_err(|e| e.to_string())?;
                    let text = std::str::from_utf8(buffer.bytes()).map_err(|e| e.to_string())?;
                    tellegen::geo::apply_aux_substation_locations(network, text)?;
                }
            }
            let network =
                crate::balanced_study_network(module.value()).ok_or("missing balanced network")?;
            let payload = crate::ingest_value(network, module.diagnostics(), Vec::new(), None)?;
            (Kind::Balanced, crate::with_module_json(payload, module)?)
        } else {
            return Err(format!(
                "collection selection holds {}, which Tellegen cannot view or solve",
                module.value().type_name()
            ));
        };
        let origin = record(origin)?;
        let output = match kind {
            Kind::Balanced => Imported::Balanced {
                payload,
                origin,
                dependencies: result.dependencies,
                incomplete: result.incomplete,
            },
            Kind::Multiconductor => Imported::Multiconductor {
                payload,
                origin,
                dependencies: result.dependencies,
                incomplete: result.incomplete,
            },
        };
        serde_json::to_value(output).map_err(|e| e.to_string())
    }
}

fn check_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 4096 {
        return Err("collection name must contain 1 to 4096 UTF-8 bytes".into());
    }
    Ok(())
}

fn selection(text: &str) -> Result<CollectionSelection, String> {
    if text.len() > MAX_SELECTION_BYTES {
        return Err("collection selection exceeds its metadata limit".into());
    }
    serde_json::from_str(text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> CaseCollection {
        let day = "New Circuit.shared basekv=12.47 bus1=source\nRedirect ../shared/line.inc\nNew Load.day bus1=load phases=3 conn=wye kv=12.47 kw=30 kvar=6\n";
        let night = day
            .replace("Load.day", "Load.night")
            .replace("kw=30", "kw=15");
        let line =
            "New Line.l bus1=source bus2=load phases=3 r1=0.1 x1=0.2 r0=0.3 x0=0.4 length=1\n";
        CaseCollection(Collection::from_files("study", [
            ("masters/day.dss", day), ("masters/night.dss", &night),
            ("shared/line.inc", line),
            ("missing.dss", "New Circuit.broken basekv=12.47\nRedirect absent.inc\n"),
            ("comparison.m", "mpc.version = '2';\nmpc.baseMVA = 100;\nmpc.bus = [1 3 0 0 0 0 1 1 0 110 1 1.1 0.9];\nmpc.gen = [1 0 0 100 -100 1 100 1 100 0];\nmpc.branch = [];\n"),
        ].into_iter().map(|(name, text)| (ArtifactPath::new(name).unwrap(), Arc::from(text.as_bytes()))), CollectionLimits::default()).unwrap())
    }

    #[test]
    fn inspection_preserves_upstream_records_and_nullable_selection_fields() {
        let collection = tree();
        let before = json!({
            "name": collection.0.name(),
            "entries": collection.0.entries(),
            "candidates": collection.0.inspect().unwrap(),
            "original_zip": collection.0.original_zip().is_some(),
            "limits": { "files": MAX_FILES, "expanded_bytes": MAX_BYTES },
        });
        assert_eq!(collection.inspect_value().unwrap(), before);
        let selection = CollectionSelection::file("comparison.m");
        let imported = collection
            .ingest_value(&serde_json::to_string(&selection).unwrap())
            .unwrap();
        assert_eq!(
            imported["origin"],
            serde_json::to_value(collection.0.selection_origin(&selection)).unwrap()
        );
        assert!(imported["origin"]["selection"]
            .as_object()
            .unwrap()
            .contains_key("root"));
        assert!(imported["origin"]["selection"]["root"].is_null());
    }

    fn select(path: &str) -> String {
        serde_json::to_string(&CollectionSelection::file(path)).unwrap()
    }

    #[test]
    fn nested_zip_keeps_sources_separate_and_records_the_container() {
        let child = tree();
        let bytes = child
            .0
            .source_bundle(
                &CollectionSelection::file("masters/day.dss"),
                SourceBundleScope::Project,
            )
            .unwrap()
            .deterministic_zip(CollectionLimits::default().archive)
            .unwrap();
        let parent =
            CaseCollection::files("outer", "[\"inner.zip\"]", &[0, bytes.len() as u32], &bytes)
                .unwrap();
        let nested = CaseCollection(parent.0.open_nested("inner.zip").unwrap());
        let imported = nested.ingest_value(&select("masters/day.dss")).unwrap();
        assert_eq!(
            imported["origin"]["origin"]["containers"],
            json!(["inner.zip"])
        );
        assert_eq!(imported["payload"]["mc_pf_supported"], true);
        assert!(nested.ingest_value(&select("../inner.zip")).is_err());
        assert!(parent.ingest_value(&select("masters/day.dss")).is_err());
        drop(nested);
        assert!(parent.inspect_value().is_ok());
    }

    #[test]
    fn advertised_batch_fits_cumulative_work_budget_with_large_shared_project() {
        let template = tree();
        let selected = template
            .0
            .source(&CollectionSelection::file("comparison.m"))
            .unwrap();
        let case = selected.primary_buffer().unwrap();
        let mut bytes = case.bytes().to_vec();
        let case_len = bytes.len() as u32;
        bytes.resize(bytes.len() + (9 << 20), b'x');
        let session = CaseCollection::files(
            "batch",
            r#"["case.m","shared.bin"]"#,
            &[0, case_len, bytes.len() as u32],
            &bytes,
        )
        .unwrap();
        session.inspect_value().unwrap();
        for _ in 0..32 {
            assert_eq!(
                session.ingest_value(&select("case.m")).unwrap()["kind"],
                "balanced"
            );
        }
    }

    #[test]
    fn autodetected_aux_keeps_locations_through_collection_and_ir_reimport() {
        let aux = b"DATA (Substation, [SubNum, Latitude, Longitude])\n{\n12 35.0 -81.0\n}\n\
                    DATA (Bus, [BusNum, BusName, BusNomVolt, BusSlack, SubNum])\n{\n\
                    1 \"ALPHA\" 138 \"YES\" 12\n}\n";
        let session =
            CaseCollection::files("locations", "[\"case.aux\"]", &[0, aux.len() as u32], aux)
                .unwrap();
        let imported = session.ingest_value(&select("case.aux")).unwrap();
        assert_eq!(imported["payload"]["view"]["buses"][0]["lon"], -81.0);
        assert_eq!(imported["payload"]["view"]["buses"][0]["lat"], 35.0);
        let saved = imported["payload"]["module_json"]
            .as_str()
            .unwrap()
            .as_bytes();
        let restored = CaseCollection::files(
            "saved",
            "[\"case.pio.json\"]",
            &[0, saved.len() as u32],
            saved,
        )
        .unwrap();
        let candidate = restored.0.inspect().unwrap().remove(0);
        let reimported = restored
            .ingest_value(&serde_json::to_string(&candidate.selection).unwrap())
            .unwrap();
        assert_eq!(reimported["payload"]["view"], imported["payload"]["view"]);
    }

    #[test]
    fn zip_and_tree_preserve_overlapping_masters_and_mixed_cases() {
        let tree = tree();
        let bytes = tree
            .0
            .source_bundle(
                &CollectionSelection::file("masters/day.dss"),
                SourceBundleScope::Project,
            )
            .unwrap()
            .deterministic_zip(CollectionLimits::default().archive)
            .unwrap();
        let zip = CaseCollection::zip("study.zip", &bytes).unwrap();
        assert_eq!(&*zip.0.original_zip().unwrap(), &bytes);
        assert_eq!(
            zip.inspect_value().unwrap()["candidates"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
        for order in [["day", "night"], ["night", "day"]] {
            for master in order {
                let selected = select(&format!("masters/{master}.dss"));
                let a = tree.ingest_value(&selected).unwrap();
                let b = zip.ingest_value(&selected).unwrap();
                assert_eq!(a["kind"], "multiconductor");
                assert_eq!(a["dependencies"], json!(["shared/line.inc"]));
                assert_eq!(a["incomplete"], false);
                assert_eq!(a["payload"]["n_load"], 1);
                assert_eq!(a["payload"]["graph"], b["payload"]["graph"]);
                assert_eq!(a["payload"]["mc_pf_supported"], true);
                let restored = crate::read_module_bytes(
                    a["payload"]["module_json"].as_str().unwrap().as_bytes(),
                )
                .unwrap();
                assert!(tellegen::validate_mc_module(&restored).is_ok());
            }
        }
        let imported = zip.ingest_value(&select("comparison.m")).unwrap();
        drop(zip);
        assert_eq!(imported["kind"], "balanced");
        let saved = imported["payload"]["module_json"].as_str().unwrap();
        assert!(crate::read_module_bytes(saved.as_bytes()).is_ok());
    }

    #[test]
    fn incomplete_invalid_and_unsupported_cases_do_not_poison_neighbors() {
        let tree = tree();
        let missing = tree.ingest_value(&select("missing.dss")).unwrap();
        assert_eq!(missing["incomplete"], true);
        assert_eq!(missing["payload"]["mc_pf_supported"], false);
        assert!(tree.ingest_value(&select("../outside.dss")).is_err());
        assert!(tree.ingest_value(&select("comparison.m")).is_ok());
        let mut confined = CollectionSelection::file("day.dss").format("dss");
        confined.root = Some("masters".into());
        assert!(tree
            .ingest_value(&serde_json::to_string(&confined).unwrap())
            .is_err());
        assert!(tree.ingest_value(&select("masters/night.dss")).is_ok());
    }

    #[test]
    fn binary_tree_framing_and_namespace_fail_closed() {
        for offsets in [vec![], vec![1, 1], vec![0, 2], vec![0, 1, 0]] {
            assert!(CaseCollection::files("study", "[\"x\"]", &offsets, &[0]).is_err());
        }
        for names in ["[\"../outside\"]", "[\"a\",\"a\"]", "[\"a\",\"a/b\"]"] {
            let count: Vec<String> = serde_json::from_str(names).unwrap();
            let offsets = vec![0; count.len() + 1];
            assert!(CaseCollection::files("study", names, &offsets, &[]).is_err());
        }
        assert!(CaseCollection::zip("study.zip", b"not a zip").is_err());
        let ok =
            CaseCollection::files("binary", "[\"opaque.bin\"]", &[0, 3], &[0, 255, 1]).unwrap();
        assert_eq!(ok.0.entries()[0].bytes, 3);
    }
}
