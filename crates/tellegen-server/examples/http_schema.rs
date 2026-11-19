//! Generate schemas from the records serialized by the HTTP server.
fn main() {
    let schema = tellegen_server::http_result_schema();
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
