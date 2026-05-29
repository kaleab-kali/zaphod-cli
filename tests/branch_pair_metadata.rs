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

    let json_list = zaphod(dir.path(), ["list", "--json"]);
    assert_success(&json_list);
    let pairs: serde_json::Value = serde_json::from_slice(&json_list.stdout).expect("list json");
    assert_eq!(
        pairs,
        json!([
            {
                "name": "default",
                "left": "feature/api",
                "right": "feature/ui",
            }
        ])
    );

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

    let empty_json_list = zaphod(dir.path(), ["list", "--json"]);
    assert_success(&empty_json_list);
    let pairs: serde_json::Value =
        serde_json::from_slice(&empty_json_list.stdout).expect("empty list json");
    assert_eq!(pairs, json!([]));
}

#[test]
fn rename_updates_pair_name() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);

    let rename = zaphod(dir.path(), ["rename", "default", "api"]);

    assert_success(&rename);
    assert_stdout_contains(
        &rename,
        "Renamed pair 'default' to 'api': feature/api <-> feature/ui",
    );

    let list = zaphod(dir.path(), ["list", "--json"]);
    assert_success(&list);
    let pairs: serde_json::Value = serde_json::from_slice(&list.stdout).expect("list json");
    assert_eq!(
        pairs,
        json!([
            {
                "name": "api",
                "left": "feature/api",
                "right": "feature/ui",
            }
        ])
    );

    let renamed_status = zaphod(dir.path(), ["status", "--name", "api"]);
    assert_success(&renamed_status);
    assert_stdout_contains(&renamed_status, "Pair: api");

    let default_status = zaphod(dir.path(), ["status"]);
    assert!(!default_status.status.success());
    assert_eq!(default_status.status.code(), Some(2));
    assert_stderr_contains(&default_status, "pair 'default' was not found");
}

#[test]
fn rename_rejects_existing_pair_name() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let default_pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&default_pair);
    let api_pair = zaphod(dir.path(), ["pair", "main", "feature/ui", "--name", "api"]);
    assert_success(&api_pair);

    let rename = zaphod(dir.path(), ["rename", "default", "api"]);

    assert!(!rename.status.success());
    assert_eq!(rename.status.code(), Some(2));
    assert_stderr_contains(&rename, "pair 'api' already exists");

    let list = zaphod(dir.path(), ["list", "--json"]);
    assert_success(&list);
    let pairs: serde_json::Value = serde_json::from_slice(&list.stdout).expect("list json");
    assert_eq!(
        pairs,
        json!([
            {
                "name": "api",
                "left": "main",
                "right": "feature/ui",
            },
            {
                "name": "default",
                "left": "feature/api",
                "right": "feature/ui",
            }
        ])
    );
}

#[test]
fn rename_rejects_invalid_new_pair_name() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);

    let rename = zaphod(dir.path(), ["rename", "default", "bad/name"]);

    assert!(!rename.status.success());
    assert_eq!(rename.status.code(), Some(2));
    assert_stderr_contains(
        &rename,
        "pair name 'bad/name' must contain only letters, numbers, '.', '_', or '-'",
    );

    let default_status = zaphod(dir.path(), ["status"]);
    assert_success(&default_status);
    assert_stdout_contains(&default_status, "Pair: default");
}

#[test]
fn status_all_reports_every_pair() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let default_pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&default_pair);
    let api_pair = zaphod(dir.path(), ["pair", "main", "feature/ui", "--name", "api"]);
    assert_success(&api_pair);

    let status = zaphod(dir.path(), ["status", "--all"]);

    assert_success(&status);
    assert_stdout_contains(&status, "Pair: api");
    assert_stdout_contains(&status, "Branches: main <-> feature/ui");
    assert_stdout_contains(
        &status,
        "Switch: not available (current branch is not part of pair)",
    );
    assert_stdout_contains(&status, "Pair: default");
    assert_stdout_contains(&status, "Other: feature/ui");
    assert_stdout_contains(&status, "Switch: allowed");

    let json_status = zaphod(dir.path(), ["status", "--all", "--json"]);
    assert_success(&json_status);
    let statuses: serde_json::Value =
        serde_json::from_slice(&json_status.stdout).expect("status all json");
    assert_eq!(
        statuses,
        json!([
            {
                "pair": "api",
                "left": "main",
                "right": "feature/ui",
                "current": "feature/api",
                "active": false,
                "other": null,
                "left_exists": true,
                "right_exists": true,
                "worktree": "clean",
                "git_state": "ready",
                "switch_allowed": false,
                "refusal_reasons": ["current_branch_not_paired"],
            },
            {
                "pair": "default",
                "left": "feature/api",
                "right": "feature/ui",
                "current": "feature/api",
                "active": true,
                "other": "feature/ui",
                "left_exists": true,
                "right_exists": true,
                "worktree": "clean",
                "git_state": "ready",
                "switch_allowed": true,
                "refusal_reasons": [],
            }
        ])
    );
}

#[test]
fn status_all_reports_empty_metadata() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());

    let status = zaphod(dir.path(), ["status", "--all"]);
    assert_success(&status);
    assert_stdout_contains(&status, "No branch pairs configured.");

    let json_status = zaphod(dir.path(), ["status", "--all", "--json"]);
    assert_success(&json_status);
    let statuses: serde_json::Value =
        serde_json::from_slice(&json_status.stdout).expect("empty status all json");
    assert_eq!(statuses, json!([]));
}

#[test]
fn preflight_json_reports_ready_pair() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);

    let preflight = zaphod(dir.path(), ["preflight", "--json"]);

    assert_success(&preflight);
    let report: serde_json::Value =
        serde_json::from_slice(&preflight.stdout).expect("preflight json");
    assert_eq!(report["ready"], true);
    assert_eq!(report["pair"], "default");
    assert_eq!(report["current"], "feature/api");
    assert_eq!(report["other"], "feature/ui");
    assert_eq!(report["worktree"], "clean");
    assert_eq!(report["git_state"], "ready");
    assert_eq!(report["switch_allowed"], true);
    assert_eq!(report["refusal_reasons"], json!([]));
    assert!(report["repository_root"].as_str().is_some());
    assert!(report["error"].is_null());
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
fn switch_dry_run_reports_target_without_switching() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);

    let output = zaphod(dir.path(), ["switch", "--dry-run"]);

    assert_success(&output);
    assert_stdout_contains(
        &output,
        "Would switch pair 'default': feature/api -> feature/ui",
    );
    assert_eq!(current_branch(dir.path()), "feature/api");
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
fn switch_refusal_can_emit_json_error() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    fs::write(dir.path().join("dirty.txt"), "local work\n").expect("write dirty file");

    let output = zaphod(dir.path(), ["--json-errors", "switch"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(3));
    let error: serde_json::Value =
        serde_json::from_slice(&output.stderr).expect("json error output");
    assert_eq!(
        error,
        json!({
            "error": {
                "kind": "switch_refused",
                "message": "refusing to switch: worktree has uncommitted changes",
                "exit_code": 3,
            }
        })
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
