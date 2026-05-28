mod support;

use std::fs;
use support::{
    TestDir, assert_stderr_contains, assert_stdout_contains, assert_success, git,
    init_repo_with_pair_branches, zaphod,
};

#[test]
fn doctor_reports_healthy_repository() {
    let dir = TestDir::new("zaphod-cli-doctor");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);

    let output = zaphod(dir.path(), ["doctor"]);

    assert_success(&output);
    assert_stdout_contains(&output, "Git: ok (git version ");
    assert_stdout_contains(&output, "Repository: ok (");
    assert_stdout_contains(&output, "Current branch: feature/api");
    assert_stdout_contains(&output, "Worktree: clean");
    assert_stdout_contains(&output, "Git state: ready");
    assert_stdout_contains(&output, "Metadata: ok (1 pair(s), ");
    assert_stdout_contains(&output, "Pairs:");
    assert_stdout_contains(&output, "- default: feature/api <-> feature/ui [ok]");
}

#[test]
fn doctor_reports_missing_pair_branch() {
    let dir = TestDir::new("zaphod-cli-doctor");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    git(dir.path(), ["branch", "-D", "feature/ui"]);

    let output = zaphod(dir.path(), ["doctor"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(4));
    assert_stdout_contains(
        &output,
        "- default: feature/api <-> feature/ui [missing right branch: feature/ui]",
    );
    assert_stderr_contains(&output, "doctor found problems");
}

#[test]
fn doctor_reports_corrupt_metadata() {
    let dir = TestDir::new("zaphod-cli-doctor");
    init_repo_with_pair_branches(dir.path());
    let metadata_dir = dir.git_dir().join("zaphod");
    fs::create_dir_all(&metadata_dir).expect("create metadata directory");
    fs::write(metadata_dir.join("pairs.toml"), "pairs = [").expect("write corrupt metadata");

    let output = zaphod(dir.path(), ["doctor"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(4));
    assert_stdout_contains(&output, "Metadata: error (failed to parse metadata file");
    assert_stderr_contains(&output, "doctor found problems");
}

#[test]
fn doctor_reports_directory_outside_git_repository() {
    let dir = TestDir::new("zaphod-cli-doctor");

    let output = zaphod(dir.path(), ["doctor"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(4));
    assert_stdout_contains(&output, "Git: ok (git version ");
    assert_stdout_contains(&output, "Repository: error (not inside a Git repository)");
    assert_stderr_contains(&output, "doctor found problems");
}
