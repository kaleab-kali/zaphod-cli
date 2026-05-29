mod support;

use serde_json::json;
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
    assert_eq!(output.status.code(), Some(2));
    assert_stderr_contains(&output, "not inside a Git repository");
}

#[test]
fn status_error_can_emit_json_error() {
    let dir = TestDir::new("zaphod-cli-safety");

    let output = zaphod(dir.path(), ["--json-errors", "status"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    let error: serde_json::Value =
        serde_json::from_slice(&output.stderr).expect("json error output");
    assert_eq!(
        error,
        json!({
            "error": {
                "kind": "not_repository",
                "message": "not inside a Git repository",
                "exit_code": 2,
            }
        })
    );
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
    assert_eq!(output.status.code(), Some(3));
    assert_stderr_contains(&output, "refusing to switch: merge is in progress");
    assert_eq!(current_branch(dir.path()), "feature/api");
}

#[test]
fn switch_dry_run_keeps_safety_refusals() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    fs::write(dir.path().join("dirty.txt"), "local work\n").expect("write dirty file");

    let output = zaphod(dir.path(), ["switch", "--dry-run"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(3));
    assert_stderr_contains(
        &output,
        "refusing to switch: worktree has uncommitted changes",
    );
    assert_eq!(current_branch(dir.path()), "feature/api");
}

#[test]
fn preflight_json_reports_dirty_refusal() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    fs::write(dir.path().join("dirty.txt"), "local work\n").expect("write dirty file");

    let output = zaphod(dir.path(), ["preflight", "--json"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(3));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("preflight json");
    assert_eq!(report["ready"], false);
    assert_eq!(report["pair"], "default");
    assert_eq!(report["current"], "feature/api");
    assert_eq!(report["other"], "feature/ui");
    assert_eq!(report["worktree"], "dirty");
    assert_eq!(report["switch_allowed"], false);
    assert_eq!(report["refusal_reasons"], json!(["dirty_worktree"]));
    assert!(report["error"].is_null());
    assert_stderr_contains(
        &output,
        "preflight failed: worktree has uncommitted changes",
    );
}

#[test]
fn preflight_json_reports_missing_pair() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());

    let output = zaphod(dir.path(), ["preflight", "--json"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("preflight json");
    assert_eq!(report["ready"], false);
    assert_eq!(report["pair"], "default");
    assert_eq!(report["switch_allowed"], false);
    assert_eq!(report["error"]["kind"], "pair_not_found");
    assert_eq!(report["error"]["message"], "pair 'default' was not found");
    assert_stderr_contains(&output, "pair 'default' was not found");
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
    assert_eq!(output.status.code(), Some(3));
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
            "switch_allowed": false,
            "refusal_reasons": ["target_branch_missing"],
        })
    );

    let output = zaphod(dir.path(), ["switch"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(3));
    assert_stderr_contains(&output, "refusing to switch: target branch is missing");
    assert_eq!(current_branch(dir.path()), "feature/api");
}
