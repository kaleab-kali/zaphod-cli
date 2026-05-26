mod support;

use std::fs;
use support::{
    TestDir, assert_stderr_contains, assert_stdout_contains, assert_success, current_branch, git,
    git_stdout, init_repo_with_pair_branches, zaphod,
};

#[test]
fn status_rejects_directory_outside_git_repository() {
    let dir = TestDir::new("zaphod-cli-safety");

    let output = zaphod(dir.path(), ["status"]);

    assert!(!output.status.success());
    assert_stderr_contains(&output, "not inside a Git repository");
}

#[test]
fn status_rejects_detached_head() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    git(dir.path(), ["switch", "--detach", "feature/api"]);

    let output = zaphod(dir.path(), ["status"]);

    assert!(!output.status.success());
    assert_stderr_contains(&output, "current Git HEAD is detached");
}

#[test]
fn status_rejects_current_branch_outside_pair() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    git(dir.path(), ["switch", "main"]);

    let output = zaphod(dir.path(), ["status"]);

    assert!(!output.status.success());
    assert_stderr_contains(
        &output,
        "current branch 'main' is not part of pair 'default'",
    );
}

#[test]
fn switch_rejects_merge_in_progress() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let head = git_stdout(dir.path(), ["rev-parse", "HEAD"]);
    fs::write(dir.git_dir().join("MERGE_HEAD"), format!("{head}\n")).expect("write MERGE_HEAD");

    let status = zaphod(dir.path(), ["status"]);
    assert_success(&status);
    assert_stdout_contains(&status, "Git state: merge in progress");
    assert_stdout_contains(&status, "Switch: refused (merge is in progress)");

    let output = zaphod(dir.path(), ["switch"]);

    assert!(!output.status.success());
    assert_stderr_contains(&output, "refusing to switch: merge is in progress");
    assert_eq!(current_branch(dir.path()), "feature/api");
}

#[test]
fn switch_rejects_rebase_in_progress() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    fs::create_dir(dir.git_dir().join("rebase-merge")).expect("create rebase marker");

    let status = zaphod(dir.path(), ["status"]);
    assert_success(&status);
    assert_stdout_contains(&status, "Git state: rebase in progress");
    assert_stdout_contains(&status, "Switch: refused (rebase is in progress)");

    let output = zaphod(dir.path(), ["switch"]);

    assert!(!output.status.success());
    assert_stderr_contains(&output, "refusing to switch: rebase is in progress");
    assert_eq!(current_branch(dir.path()), "feature/api");
}

#[test]
fn switch_rejects_missing_target_branch() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    git(dir.path(), ["branch", "-D", "feature/ui"]);

    let status = zaphod(dir.path(), ["status"]);
    assert_success(&status);
    assert_stdout_contains(&status, "Switch: refused (target branch is missing)");

    let output = zaphod(dir.path(), ["switch"]);

    assert!(!output.status.success());
    assert_stderr_contains(&output, "refusing to switch: target branch is missing");
    assert_eq!(current_branch(dir.path()), "feature/api");
}
