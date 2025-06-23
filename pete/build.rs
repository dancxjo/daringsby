use std::process::Command;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=../frontend/src/app.ts");
    println!("cargo:rerun-if-changed=../frontend/package.json");
    println!("cargo:rerun-if-changed=../frontend/tsconfig.json");

    let frontend_dir = Path::new("../frontend");

    // Install dependencies if needed and build the TypeScript
    let status = Command::new("npm")
        .arg("ci")
        .current_dir(frontend_dir)
        .status()
        .expect("failed to run npm ci");
    assert!(status.success());

    let status = Command::new("npm")
        .args(["run", "build"])
        .current_dir(frontend_dir)
        .status()
        .expect("failed to run npm build");
    assert!(status.success());
}
