//! Keep the JavaScript surface stable when the selected PowerIO release has
//! no collection API. No archive bytes are parsed or silently flattened.

use wasm_bindgen::prelude::*;

const UNAVAILABLE: &str =
    "ZIP and folder imports are unavailable in this build. Open a single case file instead.";

#[wasm_bindgen]
pub struct CaseCollection;

#[wasm_bindgen]
impl CaseCollection {
    pub fn from_zip(_name: &str, _bytes: &[u8]) -> Result<CaseCollection, JsError> {
        Err(crate::jserr(UNAVAILABLE))
    }

    pub fn from_files(
        _name: &str,
        _names_json: &str,
        _offsets: &[u32],
        _bytes: &[u8],
    ) -> Result<CaseCollection, JsError> {
        Err(crate::jserr(UNAVAILABLE))
    }

    pub fn inspect(&self) -> Result<String, JsError> {
        Err(crate::jserr(UNAVAILABLE))
    }

    pub fn ingest(&self, _selection_json: &str) -> Result<String, JsError> {
        Err(crate::jserr(UNAVAILABLE))
    }

    pub fn original_zip(&self) -> Result<Vec<u8>, JsError> {
        Err(crate::jserr(UNAVAILABLE))
    }

    pub fn source_zip(&self, _selection_json: &str, _scope: &str) -> Result<Vec<u8>, JsError> {
        Err(crate::jserr(UNAVAILABLE))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn baseline_never_advertises_collection_support() {
        assert!(!crate::collection_supported());
    }
}
