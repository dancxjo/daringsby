use include_dir::{Dir, include_dir};

/// Embedded source files for the entire workspace.
///
/// This module bundles the source tree of every crate along with Markdown
/// files so that examples and tests can inspect them at runtime.
///
/// # Examples
/// ```
/// use pete::source::{list_files, get_file};
/// let files = list_files();
/// assert!(files.contains(&"pete/src/main.rs"));
/// let main = get_file("pete/src/main.rs").unwrap();
/// assert!(main.contains("pete"));
/// ```
///
/// File paths are relative to the workspace root. This includes every
/// crate as well as Markdown documentation.
static SRC: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/..");

/// Return the list of bundled file paths.
pub fn list_files() -> Vec<&'static str> {
    fn gather<'a>(d: &'a Dir<'a>, out: &mut Vec<&'a str>) {
        for f in d.files() {
            out.push(f.path().to_str().expect("utf8 path"));
        }
        for sub in d.dirs() {
            gather(sub, out);
        }
    }

    let mut names = Vec::new();
    gather(&SRC, &mut names);
    names.sort();
    names
}

/// Retrieve a bundled file by name.
pub fn get_file(name: &str) -> Option<&'static str> {
    SRC.get_file(name).and_then(|f| f.contents_utf8())
}
