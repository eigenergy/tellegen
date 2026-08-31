//! Descriptive history entries for the modules tellegen saves.
//!
//! A saved module records the operations that produced its value as powerio
//! [`HistoryEntry`] records: one `Edit` entry per committed session edit.
//! History is descriptive — a reader gets
//! the correct value without it, and tellegen never interprets it as state; a reloaded
//! module starts a fresh session at the saved operating point.
//!
use std::collections::BTreeMap;

use powerio::{HistoryEntry, HistoryId, HistoryKind};
use serde_json::Value;

use crate::study::NetworkEdit;

/// The registered operation name of a committed demand edit.
pub const ADD_LOAD: &str = "tellegen.add_load";
/// The registered operation name of a committed rating edit.
pub const ADJUST_BRANCH_RATING: &str = "tellegen.adjust_branch_rating";

/// One descriptive entry for the `index`-th committed edit. The parameters
/// carry the edited element key exactly as the session held it (numeric id or
/// uid string) and the MW step.
pub fn edit_entry(index: usize, edit: &NetworkEdit) -> Result<HistoryEntry, String> {
    let id = HistoryId::new(format!("tellegen-edit-{index}")).map_err(|e| e.to_string())?;
    let (name, element_field, key, mw_field, mw) = match edit {
        NetworkEdit::AddLoad { bus, p_mw } => (ADD_LOAD, "bus", bus, "p_mw", *p_mw),
        NetworkEdit::AdjustBranchRating { branch, delta_mw } => (
            ADJUST_BRANCH_RATING,
            "branch",
            branch,
            "delta_mw",
            *delta_mw,
        ),
    };
    let mut parameters = BTreeMap::new();
    parameters.insert(
        element_field.to_owned(),
        serde_json::to_value(key).map_err(|e| e.to_string())?,
    );
    parameters.insert(mw_field.to_owned(), Value::from(mw));
    HistoryEntry::new(id, HistoryKind::Edit, name)
        .and_then(|entry| entry.with_parameters(parameters))
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ElementKey;

    #[test]
    fn an_edit_entry_carries_the_key_and_the_step() {
        let entry = edit_entry(
            3,
            &NetworkEdit::AddLoad {
                bus: ElementKey::Uid("buses:1".into()),
                p_mw: 20.0,
            },
        )
        .unwrap();
        assert_eq!(entry.id().as_str(), "tellegen-edit-3");
        assert_eq!(entry.kind(), HistoryKind::Edit);
        assert_eq!(entry.name(), ADD_LOAD);
        assert_eq!(entry.parameters()["bus"], "buses:1");
        assert_eq!(entry.parameters()["p_mw"], 20.0);
    }

    #[test]
    fn a_rating_entry_keeps_the_numeric_key_numeric() {
        let entry = edit_entry(
            0,
            &NetworkEdit::AdjustBranchRating {
                branch: ElementKey::Id(2),
                delta_mw: -25.0,
            },
        )
        .unwrap();
        assert_eq!(entry.name(), ADJUST_BRANCH_RATING);
        assert_eq!(entry.parameters()["branch"], 2);
        assert_eq!(entry.parameters()["delta_mw"], -25.0);
    }
}
