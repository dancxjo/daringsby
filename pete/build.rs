use std::{
    env,
    fs::{File, create_dir_all},
    io::Write,
    path::PathBuf,
};
use walkdir::WalkDir;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace = manifest_dir.parent().unwrap();
    let crates = ["pete", "psyche", "memory", "modeldb", "lingproc"];

    let mut files = Vec::new();
    for c in &crates {
        let src = workspace.join(c).join("src");
        for entry in WalkDir::new(&src) {
            let entry = entry.unwrap();
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "rs" {
                        files.push(entry.path().to_path_buf());
                    }
                }
            }
        }
    }
    files.sort();

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    create_dir_all(&out_dir).unwrap();
    let dest_path = out_dir.join("sources.rs");
    let mut out = File::create(&dest_path).unwrap();

    writeln!(out, "pub static FILES: &[(&str, &str)] = &[").unwrap();
    for path in &files {
        let rel = path.strip_prefix(workspace).unwrap();
        writeln!(
            out,
            "    (\"{}\", include_str!(\"{}\")),",
            rel.display(),
            path.display()
        )
        .unwrap();
    }
    writeln!(out, "];").unwrap();
}
