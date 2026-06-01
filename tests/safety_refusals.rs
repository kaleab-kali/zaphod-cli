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
fn handoff_json_reports_directory_outside_git_repository() {
    let dir = TestDir::new("zaphod-cli-safety");

    let output = zaphod(dir.path(), ["handoff", "--json"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("handoff json");
    assert_eq!(report["ok"], false);
    assert_eq!(report["requested_pair"], "default");
    assert!(report["repository_root"].is_null());
    assert!(report["current_branch"].is_null());
    assert_eq!(report["errors"][0]["kind"], "not_repository");
    assert_eq!(
        report["errors"][0]["message"],
        "not inside a Git repository"
    );
    assert_eq!(report["errors"][0]["exit_code"], 2);
    assert_stderr_contains(&output, "not inside a Git repository");
}

#[test]
fn handoff_rejects_invalid_stale_claim_window() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());

    let output = zaphod(
        dir.path(),
        ["handoff", "--agent", "codex", "--stale-after", "later"],
    );

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_stderr_contains(
        &output,
        "duration 'later' is invalid; use a positive number followed by s, m, h, or d",
    );
}

#[test]
fn init_rejects_invalid_or_missing_other_branch() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());

    let output = zaphod(dir.path(), ["init", "feature..ui"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_stderr_contains(&output, "branch name 'feature..ui' is invalid");

    let output = zaphod(dir.path(), ["init", "feature/missing"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_stderr_contains(&output, "branch 'feature/missing' was not found");
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
fn preflight_json_reports_claim_conflict() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let claim = zaphod(dir.path(), ["claim", "--agent", "codex"]);
    assert_success(&claim);

    let output = zaphod(dir.path(), ["preflight", "--json", "--agent", "other"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(3));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("preflight json");
    assert_eq!(report["ready"], false);
    assert_eq!(report["switch_allowed"], true);
    assert_eq!(report["claim"]["requested_agent"], "other");
    assert_eq!(report["claim"]["claim_allowed"], false);
    assert_eq!(report["claim"]["conflict"]["agent"], "codex");
    assert_eq!(report["claim"]["conflict"]["pair"], "default");
    assert_eq!(report["claim"]["conflict"]["branch"], "feature/api");
    assert_stderr_contains(
        &output,
        "pair 'default' on branch 'feature/api' is already claimed by agent 'codex'",
    );
}

#[test]
fn preflight_json_reports_metadata_lock_claim_blocker() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    fs::create_dir_all(dir.git_dir().join("zaphod").join("metadata.lock"))
        .expect("create metadata lock");

    let output = zaphod(dir.path(), ["preflight", "--json", "--agent", "codex"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(3));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("preflight json");
    assert_eq!(report["ready"], false);
    assert_eq!(report["switch_allowed"], true);
    assert_eq!(report["claim"]["requested_agent"], "codex");
    assert_eq!(report["claim"]["claim_allowed"], false);
    assert_eq!(report["claim"]["metadata_lock"]["ok"], false);
    assert_eq!(report["claim"]["metadata_lock"]["locked"], true);
    assert!(report["claim"]["conflict"].is_null());
    assert_stderr_contains(
        &output,
        "claim for agent 'codex' on pair 'default' and branch 'feature/api' is blocked",
    );
}

#[test]
fn preflight_json_reports_stale_claim_conflict() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let metadata_dir = dir.git_dir().join("zaphod");
    fs::create_dir_all(&metadata_dir).expect("create zaphod metadata directory");
    fs::write(
        metadata_dir.join("claims.toml"),
        r#"[[claims]]
agent = "codex"
pair = "default"
branch = "feature/api"
created_at_unix = 1
"#,
    )
    .expect("write claims metadata");

    let output = zaphod(
        dir.path(),
        [
            "preflight",
            "--json",
            "--agent",
            "other",
            "--stale-after",
            "1d",
        ],
    );

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(3));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("preflight json");
    assert_eq!(report["ready"], false);
    assert_eq!(report["claim"]["requested_agent"], "other");
    assert_eq!(report["claim"]["claim_allowed"], false);
    assert_eq!(report["claim"]["stale_after_seconds"], 86_400);
    assert_eq!(report["claim"]["conflict_stale"], true);
    assert_eq!(report["claim"]["conflict"]["agent"], "codex");
}

#[test]
fn preflight_rejects_invalid_stale_claim_window() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());

    let output = zaphod(
        dir.path(),
        ["preflight", "--agent", "codex", "--stale-after", "0h"],
    );

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_stderr_contains(
        &output,
        "duration '0h' is invalid; use a positive number followed by s, m, h, or d",
    );
}

#[test]
fn preflight_json_reports_wrong_expected_side() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);

    let output = zaphod(dir.path(), ["preflight", "--json", "--side", "right"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("preflight json");
    assert_eq!(report["ready"], false);
    assert_eq!(report["switch_allowed"], true);
    assert_eq!(report["expectation"]["ok"], false);
    assert_eq!(report["expectation"]["expected_side"], "right");
    assert_eq!(report["expectation"]["current_side"], "left");
    assert!(
        report["expectation"]["failures"][0]
            .as_str()
            .expect("failure message")
            .contains("is not the right side")
    );
    assert_stderr_contains(&output, "assertion failed:");
}

#[test]
fn assert_json_reports_wrong_side() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);

    let output = zaphod(dir.path(), ["assert", "--json", "--side", "right"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("assert json");
    assert_eq!(report["ok"], false);
    assert_eq!(report["pair"]["name"], "default");
    assert_eq!(report["pair"]["current_side"], "left");
    assert_eq!(report["pair"]["expected_side"], "right");
    assert!(
        report["failures"][0]
            .as_str()
            .expect("failure message")
            .contains("not the right side")
    );
    assert_stderr_contains(&output, "assertion failed:");
}

#[test]
fn assert_json_reports_wrong_branch_without_pair_requirement() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());

    let output = zaphod(dir.path(), ["assert", "--json", "--branch", "main"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("assert json");
    assert_eq!(report["ok"], false);
    assert_eq!(report["current_branch"], "feature/api");
    assert_eq!(report["expected_branch"], "main");
    assert!(report["pair"].is_null());
    assert!(
        report["failures"][0]
            .as_str()
            .expect("failure message")
            .contains("expected branch 'main'")
    );
    assert_stderr_contains(&output, "assertion failed:");
}

#[test]
fn claim_json_reports_conflicting_agent() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let claim = zaphod(dir.path(), ["claim", "--agent", "codex"]);
    assert_success(&claim);

    let output = zaphod(dir.path(), ["claim", "--json", "--agent", "other"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(3));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("claim json");
    assert_eq!(report["ok"], false);
    assert_eq!(report["status"], "conflict");
    assert_eq!(report["agent"], "other");
    assert_eq!(report["pair"], "default");
    assert_eq!(report["branch"], "feature/api");
    assert_eq!(report["conflict"]["agent"], "codex");
    assert_stderr_contains(
        &output,
        "pair 'default' on branch 'feature/api' is already claimed by agent 'codex'",
    );
}

#[test]
fn heartbeat_json_reports_conflicting_agent() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let claim = zaphod(dir.path(), ["claim", "--agent", "codex"]);
    assert_success(&claim);

    let output = zaphod(dir.path(), ["heartbeat", "--json", "--agent", "other"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(3));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("heartbeat json");
    assert_eq!(report["ok"], false);
    assert_eq!(report["status"], "conflict");
    assert_eq!(report["agent"], "other");
    assert_eq!(report["pair"], "default");
    assert_eq!(report["branch"], "feature/api");
    assert_eq!(report["conflict"]["agent"], "codex");
    assert_stderr_contains(
        &output,
        "pair 'default' on branch 'feature/api' is already claimed by agent 'codex'",
    );
}

#[test]
fn heartbeat_reports_missing_claim() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);

    let output = zaphod(dir.path(), ["heartbeat", "--agent", "codex"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_stderr_contains(
        &output,
        "no claim for agent 'codex' on pair 'default' and branch 'feature/api'",
    );
}

#[test]
fn claim_json_reports_dirty_refusal() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    fs::write(dir.path().join("dirty.txt"), "local work\n").expect("write dirty file");

    let output = zaphod(dir.path(), ["claim", "--json", "--agent", "codex"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(3));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("claim json");
    assert_eq!(report["ok"], false);
    assert_eq!(report["status"], "refused");
    assert_eq!(report["agent"], "codex");
    assert_eq!(report["refusal_reasons"], json!(["dirty_worktree"]));
    assert_stderr_contains(
        &output,
        "preflight failed: worktree has uncommitted changes",
    );
}

#[test]
fn claim_rejects_invalid_agent_name() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());

    let output = zaphod(dir.path(), ["claim", "--agent", "bad/name"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_stderr_contains(
        &output,
        "agent name 'bad/name' must contain only letters, numbers, '.', '_', or '-'",
    );
}

#[test]
fn claims_rejects_invalid_filter_values() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());

    let output = zaphod(dir.path(), ["claims", "--agent", "bad/name"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_stderr_contains(
        &output,
        "agent name 'bad/name' must contain only letters, numbers, '.', '_', or '-'",
    );

    let output = zaphod(dir.path(), ["claims", "--pair", "bad/name"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_stderr_contains(
        &output,
        "pair name 'bad/name' must contain only letters, numbers, '.', '_', or '-'",
    );

    let output = zaphod(dir.path(), ["claims", "--branch", "feature..api"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_stderr_contains(&output, "branch name 'feature..api' is invalid");

    let output = zaphod(dir.path(), ["claims", "--stale-after", "soon"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_stderr_contains(
        &output,
        "duration 'soon' is invalid; use a positive number followed by s, m, h, or d",
    );
}

#[test]
fn doctor_rejects_invalid_stale_claim_window() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());

    let output = zaphod(dir.path(), ["doctor", "--stale-after", "later"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_stderr_contains(
        &output,
        "duration 'later' is invalid; use a positive number followed by s, m, h, or d",
    );
}

#[test]
fn prune_claims_rejects_invalid_inputs() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());

    let output = zaphod(dir.path(), ["prune-claims", "--stale-after", "soon"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_stderr_contains(
        &output,
        "duration 'soon' is invalid; use a positive number followed by s, m, h, or d",
    );

    let output = zaphod(
        dir.path(),
        [
            "prune-claims",
            "--branch",
            "feature..api",
            "--stale-after",
            "1d",
        ],
    );

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_stderr_contains(&output, "branch name 'feature..api' is invalid");
}

#[test]
fn unclaim_reports_missing_claim() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());

    let output = zaphod(dir.path(), ["unclaim", "--agent", "codex"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_stderr_contains(
        &output,
        "no claim for agent 'codex' on pair 'default' and branch 'feature/api'",
    );
}

#[test]
fn unclaim_rejects_invalid_branch_name() {
    let dir = TestDir::new("zaphod-cli-safety");
    init_repo_with_pair_branches(dir.path());

    let output = zaphod(
        dir.path(),
        ["unclaim", "--agent", "codex", "--branch", "feature..api"],
    );

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert_stderr_contains(&output, "branch name 'feature..api' is invalid");
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
