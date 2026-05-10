use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("autologos_source.txt");
    let mut out_file = String::new();

    let root = Path::new("..").canonicalize().unwrap_or_else(|_| Path::new("..").to_path_buf());
    
    // We only want to look at specific directories to avoid target/ and node_modules/
    let dirs_to_search = vec!["pete", "psyche", "shared", "common", "lingproc", "frontend", "codex", "models", "xtask"];
    
    for dir in &dirs_to_search {
        println!("cargo:rerun-if-changed=../{}", dir);
    }
    println!("cargo:rerun-if-changed=../Cargo.toml");
    println!("cargo:rerun-if-changed=../justfile");

    let mut paths_to_process = Vec::new();

    for root_file in ["Cargo.toml", "justfile", "AGENTS.md", "README.md", ".env.example", "docker-compose.yml"] {
        let p = root.join(root_file);
        if p.exists() {
            paths_to_process.push(p);
        }
    }

    for dir in dirs_to_search {
        let dir_path = root.join(dir);
        if !dir_path.exists() { continue; }
        
        let mut stack = vec![dir_path];
        while let Some(path) = stack.pop() {
            if let Ok(entries) = fs::read_dir(&path) {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if entry_path.is_dir() {
                        let name = entry_path.file_name().unwrap().to_string_lossy();
                        if name == "target" || name == "node_modules" || name == "dist" || name == ".git" {
                            continue;
                        }
                        stack.push(entry_path);
                    } else {
                        let ext = entry_path.extension().unwrap_or_default().to_string_lossy();
                        let name = entry_path.file_name().unwrap().to_string_lossy();
                        if ext == "rs" || ext == "js" || ext == "md" || ext == "toml" || ext == "css" || ext == "html" || name == "Dockerfile" {
                            paths_to_process.push(entry_path);
                        }
                    }
                }
            }
        }
    }

    paths_to_process.sort();

    for path in paths_to_process {
        if let Ok(content) = fs::read_to_string(&path) {
            let rel_path = path.strip_prefix(&root).unwrap_or(&path);
            out_file.push_str(&format!("@@@FILE: {}\n", rel_path.display()));
            out_file.push_str(&content);
            out_file.push_str("\n");
        }
    }

    fs::write(&dest_path, out_file).unwrap();
}
