use std::path::PathBuf;

fn main() {
    let html = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("index.html");
    println!("cargo:rerun-if-changed={}", html.display());
}
