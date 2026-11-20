//! Generate the shared Study schema from the application-owned Rust records.

use schemars::JsonSchema;

#[derive(JsonSchema)]
#[allow(dead_code)]
struct StudySchema {
    bundle: tellegen::document::StudyBundle,
    create: tellegen::study_ops::CreateStudy,
    request: tellegen::study_ops::StudyRequest,
    response: tellegen::study_ops::StudyOperationOutput,
}

fn main() {
    // Through a JSON value so the keys print in sorted order.
    let schema = schemars::schema_for!(StudySchema).to_value();
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).expect("Study schema JSON")
    );
}
