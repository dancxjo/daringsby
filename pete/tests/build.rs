use assert_cmd::Command;

#[test]
fn build_pete() {
    Command::new("cargo")
        .args(["build", "--bin", "pete"])
        .assert()
        .success();
}
