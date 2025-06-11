use include_dir::{Dir, include_dir};

/// Embedded source files for Pete.
///
/// The module exposes helpers to list bundled files and retrieve their
/// contents as UTF-8 strings.
///
/// # Examples
/// ```
/// use pete::source::{list_files, get_file};
/// let files = list_files();
/// assert!(files.contains(&"main.rs"));
/// let main = get_file("main.rs").unwrap();
/// assert!(main.contains("pete"));
/// ```
static SRC: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src");

/// Return the list of bundled file paths.
pub fn list_files() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = SRC
        .files()
        .map(|f| f.path().to_str().expect("utf8 path"))
        .collect();
    names.sort();
    names
}

/// Retrieve a bundled file by name.
pub fn get_file(name: &str) -> Option<&'static str> {
    SRC.get_file(name).and_then(|f| f.contents_utf8())
}
