//! Error wording for `.pio.json` package reads.
//!
//! This module must not depend on `powerio-pkg`. That crate is optional and
//! gated on `sensitivity`, and the multiconductor viewer builds without it.

use std::fmt::Display;

/// powerio starts its message with this text when it rejects a `.pio.json`
/// from a format version it does not read. Only the start is stable: 0.7.x
/// ends the sentence with "this reader supports major version N", while later
/// versions name a `major.minor` lineage. Match the start, and a powerio
/// update does not break this.
pub(crate) const UNSUPPORTED_SCHEMA_VERSION: &str = "unsupported .pio.json schema_version";

/// Make a `NetworkPackage::from_json` failure ready to show to a user.
///
/// A file from a different format version is not a bad file, so tell the user
/// to save the study again. Other failures keep powerio's own text.
pub fn read_error(err: impl Display) -> String {
    let message = err.to_string();
    if message.contains(UNSUPPORTED_SCHEMA_VERSION) {
        return format!(
            "this .pio.json was written by a different version of the package format and \
             cannot be read: {message}. Open the source case again and re-save the study to \
             write a current package."
        );
    }
    format!("invalid .pio.json package: {message}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_mismatch_tells_the_user_to_re_save() {
        // The 0.7.x wording, then the wording that replaces it when the
        // envelope collapses to one version number.
        let messages = [
            "unsupported .pio.json schema_version 9.0.0; this reader supports major version 0",
            "unsupported .pio.json schema_version 0.1.1; this reader supports 0.2.x; \
             regenerate the package from its source case",
        ];
        for message in messages {
            let wrapped = read_error(message);
            assert!(wrapped.contains("re-save the study"), "got: {wrapped}");
            assert!(
                wrapped.contains(message),
                "upstream detail dropped: {wrapped}"
            );
        }
    }

    #[test]
    fn other_failures_keep_the_generic_wording() {
        let wrapped = read_error("expected value at line 1 column 1");
        assert!(
            wrapped.starts_with("invalid .pio.json package: "),
            "got: {wrapped}"
        );
        assert!(!wrapped.contains("re-save the study"), "got: {wrapped}");
    }
}
