#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tellegen::server::run_from_env().await
}

#[cfg(target_arch = "wasm32")]
fn main() {}
