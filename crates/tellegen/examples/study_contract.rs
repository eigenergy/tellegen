//! Generate the shared Study schema from the application-owned Rust records.

use schemars::JsonSchema;
use sha2::{Digest, Sha256};
use std::fmt::Write;

#[derive(JsonSchema)]
#[allow(dead_code)]
struct StudyContract {
    bundle: tellegen::document::StudyBundle,
    create: tellegen::study_ops::CreateStudy,
    request: tellegen::study_ops::StudyRequest,
    response: tellegen::study_ops::StudyOperationResult,
}

fn main() {
    let sources = [
        include_str!("../src/document.rs"),
        include_str!("../src/preparation.rs"),
        include_str!("../src/objective.rs"),
        include_str!("../src/exploration.rs"),
        include_str!("../src/study_ops.rs"),
        include_str!("../src/api.rs"),
        include_str!("../src/sens/contract.rs"),
        include_str!("../src/solve.rs"),
    ];
    let mut hash = Sha256::new();
    for source in sources {
        hash.update(source.as_bytes());
    }
    let mut schema =
        serde_json::to_value(schemars::schema_for!(StudyContract)).expect("Study schema");
    let digest = hash.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        write!(&mut hex, "{byte:02x}").expect("write to String");
    }
    schema["x-rust-source-sha256"] = hex.into();
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).expect("Study schema JSON")
    );
}
