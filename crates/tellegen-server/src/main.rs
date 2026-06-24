//! The tellegen demo server binary: load the staged cases and serve the HTTP API
//! and the static frontend. All solver work lives in the `tellegen` engine; this
//! binary is the native HTTP transport.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tellegen_server::run_from_env().await
}
