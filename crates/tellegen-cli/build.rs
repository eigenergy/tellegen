use std::path::PathBuf;

fn main() {
    let lockfile = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest directory"))
        .join("../../Cargo.lock");
    println!("cargo:rerun-if-changed={}", lockfile.display());
    let text = std::fs::read_to_string(&lockfile).expect("read workspace Cargo.lock");
    let version = text
        .split("[[package]]")
        .find_map(|package| {
            let mut name = None;
            let mut version = None;
            for line in package.lines().map(str::trim) {
                if let Some(value) = line.strip_prefix("name = \"") {
                    name = value.strip_suffix('"');
                } else if let Some(value) = line.strip_prefix("version = \"") {
                    version = value.strip_suffix('"');
                }
            }
            (name == Some("powerio")).then_some(version).flatten()
        })
        .expect("powerio package version in Cargo.lock");
    println!("cargo:rustc-env=TELLEGEN_POWERIO_VERSION={version}");
}
