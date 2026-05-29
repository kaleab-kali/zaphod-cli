mod support;

use support::{TestDir, assert_stdout_contains, assert_success, zaphod};

#[test]
fn generates_bash_completions() {
    let dir = TestDir::new("zaphod-cli-completions");

    let output = zaphod(dir.path(), ["completions", "bash"]);

    assert_success(&output);
    assert_stdout_contains(&output, "_zaphod()");
    assert_stdout_contains(&output, "assert");
    assert_stdout_contains(&output, "claim");
    assert_stdout_contains(&output, "claims");
    assert_stdout_contains(&output, "completions");
    assert_stdout_contains(&output, "status");
    assert_stdout_contains(&output, "switch");
    assert_stdout_contains(&output, "unclaim");
}
