mod support;

use serde_json::json;
use std::fs;
use support::{
    TestDir, assert_stderr_contains, assert_stdout_contains, assert_success, current_branch,
    init_repo_with_pair_branches, zaphod,
};

#[test]
fn pair_list_and_unpair_update_repo_metadata() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());

    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    assert_stdout_contains(&pair, "Paired 'default': feature/api <-> feature/ui");

    let metadata =
        fs::read_to_string(dir.git_dir().join("zaphod").join("pairs.toml")).expect("read metadata");
    assert!(metadata.contains("name = \"default\""));
    assert!(metadata.contains("left = \"feature/api\""));
    assert!(metadata.contains("right = \"feature/ui\""));

    let list = zaphod(dir.path(), ["list"]);
    assert_success(&list);
    assert_stdout_contains(&list, "default: feature/api <-> feature/ui");

    let status = zaphod(dir.path(), ["status"]);
    assert_success(&status);
    assert_stdout_contains(&status, "Pair: default");
    assert_stdout_contains(&status, "Current: feature/api");
    assert_stdout_contains(&status, "Other: feature/ui");
    assert_stdout_contains(&status, "Worktree: clean");
    assert_stdout_contains(&status, "Git state: ready");
    assert_stdout_contains(&status, "Switch: allowed");

    let json_status = zaphod(dir.path(), ["status", "--json"]);
    assert_success(&json_status);
    let status: serde_json::Value =
        serde_json::from_slice(&json_status.stdout).expect("status json");
    assert_eq!(
        status,
        json!({
            "pair": "default",
            "current": "feature/api",
            "other": "feature/ui",
            "worktree": "clean",
            "git_state": "ready",
            "switch_allowed": true,
            "refusal_reasons": [],
        })
    );

    let unpair = zaphod(dir.path(), ["unpair"]);
    assert_success(&unpair);
    assert_stdout_contains(
        &unpair,
        "Removed pair 'default': feature/api <-> feature/ui",
    );

    let empty_list = zaphod(dir.path(), ["list"]);
    assert_success(&empty_list);
    assert_stdout_contains(&empty_list, "No branch pairs configured.");
}

#[test]
fn status_reports_dirty_worktree_refusal() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    fs::write(dir.path().join("dirty.txt"), "local work\n").expect("write dirty file");

    let status = zaphod(dir.path(), ["status"]);

    assert_success(&status);
    assert_stdout_contains(&status, "Worktree: dirty");
    assert_stdout_contains(
        &status,
        "Switch: refused (worktree has uncommitted changes)",
    );
}

#[test]
fn switch_moves_to_other_branch_when_clean() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);

    let output = zaphod(dir.path(), ["switch"]);

    assert_success(&output);
    assert_stdout_contains(
        &output,
        "Switched pair 'default': feature/api -> feature/ui",
    );
    assert_eq!(current_branch(dir.path()), "feature/ui");
}

#[test]
fn switch_refuses_dirty_worktree() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    fs::write(dir.path().join("dirty.txt"), "local work\n").expect("write dirty file");

    let output = zaphod(dir.path(), ["switch"]);

    assert!(!output.status.success());
    assert_stderr_contains(
        &output,
        "refusing to switch: worktree has uncommitted changes",
    );
    assert_eq!(current_branch(dir.path()), "feature/api");
}

#[test]
fn pair_rejects_missing_branch() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());

    let output = zaphod(dir.path(), ["pair", "feature/api", "missing"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_stderr_contains(&output, "branch 'missing' was not found");
}

#[test]
fn pair_rejects_invalid_branch_name() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());

    let output = zaphod(dir.path(), ["pair", "feature/api", "feature..ui"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_stderr_contains(&output, "branch name 'feature..ui' is invalid");
}
