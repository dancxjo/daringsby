use pete::source::{get_file, list_files};

#[test]
fn lists_main_rs() {
    let files = list_files();
    assert!(files.iter().any(|f| *f == "pete/src/main.rs"));
}

#[test]
fn loads_main_rs() {
    let expected = include_str!("../src/main.rs");
    assert_eq!(get_file("pete/src/main.rs"), Some(expected));
}
