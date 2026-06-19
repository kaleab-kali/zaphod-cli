mod support;

use serde_json::json;
use std::fs;
use support::{
    TestDir, assert_stderr_contains, assert_stdout_contains, assert_success, current_branch, git,
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
fn pair_refuses_existing_metadata_lock() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    fs::create_dir_all(dir.git_dir().join("zaphod").join("metadata.lock"))
        .expect("create metadata lock");

    let output = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    assert_stderr_contains(&output, "metadata is locked by another Zaphod process");
}

#[test]
fn init_pairs_current_branch_with_other_branch() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    assert_eq!(current_branch(dir.path()), "feature/api");

    let init = zaphod(dir.path(), ["init", "feature/ui"]);

    assert_success(&init);
    assert_stdout_contains(
        &init,
        "Initialized pair 'default': feature/api <-> feature/ui",
    );

    let list = zaphod(dir.path(), ["list", "--json"]);
    assert_success(&list);
    let pairs: serde_json::Value = serde_json::from_slice(&list.stdout).expect("list json");
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
    assert_stdout_contains(&status, "Current: feature/api");
    assert_stdout_contains(&status, "Other: feature/ui");
}

#[test]
fn init_json_reports_initialized_pair() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());

    let output = zaphod(dir.path(), ["init", "feature/ui", "--json"]);

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("init json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["action"], "initialized");
    assert_eq!(report["pair"]["name"], "default");
    assert_eq!(report["pair"]["left"], "feature/api");
    assert_eq!(report["pair"]["right"], "feature/ui");
    assert!(report["previous_pair"].is_null());
    assert!(report["repository_root"].as_str().is_some());
    assert!(
        report["pairs_path"]
            .as_str()
            .expect("pairs path")
            .ends_with("pairs.toml")
    );
}

#[test]
fn pair_mutation_commands_emit_json_reports() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());

    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui", "--json"]);
    assert_success(&pair);
    let report: serde_json::Value = serde_json::from_slice(&pair.stdout).expect("pair json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["action"], "created");
    assert_eq!(report["pair"]["name"], "default");
    assert_eq!(report["pair"]["left"], "feature/api");
    assert_eq!(report["pair"]["right"], "feature/ui");
    assert!(report["previous_pair"].is_null());
    assert!(report["repository_root"].as_str().is_some());
    assert!(
        report["pairs_path"]
            .as_str()
            .expect("pairs path")
            .ends_with("pairs.toml")
    );

    let update = zaphod(dir.path(), ["pair", "main", "feature/ui", "--json"]);
    assert_success(&update);
    let report: serde_json::Value = serde_json::from_slice(&update.stdout).expect("update json");
    assert_eq!(report["action"], "updated");
    assert_eq!(report["pair"]["left"], "main");
    assert_eq!(report["previous_pair"]["name"], "default");
    assert_eq!(report["previous_pair"]["left"], "feature/api");
    assert_eq!(report["previous_pair"]["right"], "feature/ui");

    let rename = zaphod(dir.path(), ["rename", "default", "api", "--json"]);
    assert_success(&rename);
    let report: serde_json::Value = serde_json::from_slice(&rename.stdout).expect("rename json");
    assert_eq!(report["action"], "renamed");
    assert_eq!(report["pair"]["name"], "api");
    assert_eq!(report["pair"]["left"], "main");
    assert_eq!(report["previous_pair"]["name"], "default");

    let unpair = zaphod(dir.path(), ["unpair", "--name", "api", "--json"]);
    assert_success(&unpair);
    let report: serde_json::Value = serde_json::from_slice(&unpair.stdout).expect("unpair json");
    assert_eq!(report["action"], "removed");
    assert_eq!(report["pair"]["name"], "api");
    assert_eq!(report["pair"]["left"], "main");
    assert!(report["previous_pair"].is_null());

    let list = zaphod(dir.path(), ["list", "--json"]);
    assert_success(&list);
    let pairs: serde_json::Value = serde_json::from_slice(&list.stdout).expect("list json");
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
fn preflight_json_reports_claim_readiness_for_agent() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);

    let preflight = zaphod(dir.path(), ["preflight", "--json", "--agent", "codex"]);

    assert_success(&preflight);
    let report: serde_json::Value =
        serde_json::from_slice(&preflight.stdout).expect("preflight json");
    assert_eq!(report["ready"], true);
    assert_eq!(report["claim"]["requested_agent"], "codex");
    assert_eq!(report["claim"]["claim_allowed"], true);
    assert_eq!(report["claim"]["metadata_lock"]["ok"], true);
    assert_eq!(report["claim"]["metadata_lock"]["locked"], false);
    assert!(report["claim"]["conflict"].is_null());
}

#[test]
fn preflight_json_requires_existing_claim_for_agent() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let claim = zaphod(dir.path(), ["claim", "--agent", "codex"]);
    assert_success(&claim);

    let preflight = zaphod(
        dir.path(),
        ["preflight", "--json", "--agent", "codex", "--require-claim"],
    );

    assert_success(&preflight);
    let report: serde_json::Value =
        serde_json::from_slice(&preflight.stdout).expect("preflight json");
    assert_eq!(report["ready"], true);
    assert_eq!(report["claim"]["requested_agent"], "codex");
    assert_eq!(report["claim"]["claim_allowed"], true);
    assert_eq!(report["claim"]["claim_required"], true);
    assert_eq!(report["claim"]["claim_owned"], true);
    assert_eq!(report["claim"]["owned_claim"]["agent"], "codex");
    assert_eq!(report["claim"]["owned_claim"]["pair"], "default");
    assert_eq!(report["claim"]["owned_claim"]["branch"], "feature/api");
    assert!(report["claim"]["conflict"].is_null());
}

#[test]
fn preflight_json_reports_expected_branch_and_side() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);

    let preflight = zaphod(
        dir.path(),
        [
            "preflight",
            "--json",
            "--branch",
            "feature/api",
            "--side",
            "left",
        ],
    );

    assert_success(&preflight);
    let report: serde_json::Value =
        serde_json::from_slice(&preflight.stdout).expect("preflight json");
    assert_eq!(report["ready"], true);
    assert_eq!(report["expectation"]["ok"], true);
    assert_eq!(report["expectation"]["expected_branch"], "feature/api");
    assert_eq!(report["expectation"]["expected_side"], "left");
    assert_eq!(report["expectation"]["current_side"], "left");
    assert_eq!(report["expectation"]["failures"], json!([]));
}

#[test]
fn assert_json_passes_for_expected_pair_branch_and_side() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);

    let output = zaphod(
        dir.path(),
        [
            "assert",
            "--json",
            "--pair",
            "default",
            "--branch",
            "feature/api",
            "--side",
            "left",
        ],
    );

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("assert json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["current_branch"], "feature/api");
    assert_eq!(report["expected_branch"], "feature/api");
    assert_eq!(report["pair"]["name"], "default");
    assert_eq!(report["pair"]["left"], "feature/api");
    assert_eq!(report["pair"]["right"], "feature/ui");
    assert_eq!(report["pair"]["configured"], true);
    assert_eq!(report["pair"]["current_side"], "left");
    assert_eq!(report["pair"]["expected_side"], "left");
    assert_eq!(report["failures"], json!([]));
    assert!(report["repository_root"].as_str().is_some());
}

#[test]
fn assert_json_requires_existing_claim_for_agent_while_dirty() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let claim = zaphod(dir.path(), ["claim", "--agent", "codex"]);
    assert_success(&claim);
    fs::write(dir.path().join("dirty.txt"), "local work\n").expect("write dirty file");

    let output = zaphod(
        dir.path(),
        [
            "assert",
            "--json",
            "--pair",
            "default",
            "--side",
            "left",
            "--agent",
            "codex",
            "--require-claim",
        ],
    );

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("assert json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["current_branch"], "feature/api");
    assert_eq!(report["pair"]["name"], "default");
    assert_eq!(report["pair"]["current_side"], "left");
    assert_eq!(report["claim"]["requested_agent"], "codex");
    assert_eq!(report["claim"]["claim_allowed"], true);
    assert_eq!(report["claim"]["claim_required"], true);
    assert_eq!(report["claim"]["claim_owned"], true);
    assert_eq!(report["claim"]["owned_claim"]["agent"], "codex");
    assert_eq!(report["claim"]["owned_claim"]["branch"], "feature/api");
    assert!(report["claim"]["conflict"].is_null());
    assert_eq!(report["failures"], json!([]));
}

#[test]
fn claim_json_records_current_pair_and_branch() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);

    let output = zaphod(dir.path(), ["claim", "--json", "--agent", "codex"]);

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("claim json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["status"], "claimed");
    assert_eq!(report["agent"], "codex");
    assert_eq!(report["pair"], "default");
    assert_eq!(report["branch"], "feature/api");
    assert_eq!(report["claim"]["agent"], "codex");
    assert_eq!(report["claim"]["pair"], "default");
    assert_eq!(report["claim"]["branch"], "feature/api");
    assert!(report["claim"]["created_at_unix"].as_u64().is_some());
    assert!(report["conflict"].is_null());
    assert_eq!(report["refusal_reasons"], json!([]));
    assert!(
        report["claims_path"]
            .as_str()
            .expect("claims path")
            .ends_with("claims.toml")
    );

    let claims = zaphod(dir.path(), ["claims", "--json"]);
    assert_success(&claims);
    let report: serde_json::Value = serde_json::from_slice(&claims.stdout).expect("claims json");
    assert_eq!(report["claims"][0]["agent"], "codex");
    assert_eq!(report["claims"][0]["pair"], "default");
    assert_eq!(report["claims"][0]["branch"], "feature/api");
}

#[test]
fn claim_json_records_note_and_claims_reports_it() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);

    let output = zaphod(
        dir.path(),
        [
            "claim",
            "--json",
            "--agent",
            "codex",
            "--note",
            "implementing API handler",
        ],
    );

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("claim json");
    assert_eq!(report["claim"]["agent"], "codex");
    assert_eq!(report["claim"]["note"], "implementing API handler");

    let claims = zaphod(dir.path(), ["claims", "--json"]);
    assert_success(&claims);
    let report: serde_json::Value = serde_json::from_slice(&claims.stdout).expect("claims json");
    assert_eq!(report["claims"][0]["agent"], "codex");
    assert_eq!(report["claims"][0]["note"], "implementing API handler");
}

#[test]
fn claim_json_can_clear_existing_claim_note() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let claim = zaphod(
        dir.path(),
        [
            "claim",
            "--agent",
            "codex",
            "--note",
            "implementing API handler",
        ],
    );
    assert_success(&claim);

    let output = zaphod(
        dir.path(),
        ["claim", "--json", "--agent", "codex", "--clear-note"],
    );

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("claim json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["status"], "claimed");
    assert!(report["claim"]["note"].is_null());

    let claims = zaphod(dir.path(), ["claims", "--json"]);
    assert_success(&claims);
    let report: serde_json::Value = serde_json::from_slice(&claims.stdout).expect("claims json");
    assert!(report["claims"][0]["note"].is_null());
}

#[test]
fn claim_json_records_expected_branch_and_side() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);

    let output = zaphod(
        dir.path(),
        [
            "claim",
            "--json",
            "--agent",
            "codex",
            "--branch",
            "feature/api",
            "--side",
            "left",
        ],
    );

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("claim json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["status"], "claimed");
    assert_eq!(report["expectation"]["ok"], true);
    assert_eq!(report["expectation"]["expected_branch"], "feature/api");
    assert_eq!(report["expectation"]["expected_side"], "left");
    assert_eq!(report["expectation"]["current_side"], "left");
    assert_eq!(report["expectation"]["failures"], json!([]));
    assert_eq!(report["claim"]["agent"], "codex");
    assert_eq!(report["claim"]["branch"], "feature/api");
}

#[test]
fn heartbeat_json_refreshes_existing_claim_while_dirty() {
    let dir = TestDir::new("zaphod-cli-metadata");
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
    .expect("write old claim metadata");
    fs::write(dir.path().join("dirty.txt"), "local work\n").expect("write dirty file");

    let output = zaphod(dir.path(), ["heartbeat", "--json", "--agent", "codex"]);

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("heartbeat json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["status"], "refreshed");
    assert_eq!(report["agent"], "codex");
    assert_eq!(report["pair"], "default");
    assert_eq!(report["branch"], "feature/api");
    assert_eq!(report["claim"]["agent"], "codex");
    assert_eq!(report["claim"]["pair"], "default");
    assert_eq!(report["claim"]["branch"], "feature/api");
    assert!(
        report["claim"]["created_at_unix"]
            .as_u64()
            .expect("claim timestamp")
            > 1
    );

    let stale_claims = zaphod(dir.path(), ["claims", "--json", "--stale-after", "1d"]);
    assert_success(&stale_claims);
    let report: serde_json::Value =
        serde_json::from_slice(&stale_claims.stdout).expect("claims json");
    assert_eq!(report["claims"], json!([]));
}

#[test]
fn heartbeat_json_preserves_existing_claim_note() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let claim = zaphod(
        dir.path(),
        [
            "claim",
            "--agent",
            "codex",
            "--note",
            "implementing API handler",
        ],
    );
    assert_success(&claim);

    let output = zaphod(dir.path(), ["heartbeat", "--json", "--agent", "codex"]);

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("heartbeat json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["status"], "refreshed");
    assert_eq!(report["claim"]["note"], "implementing API handler");
}

#[test]
fn heartbeat_json_can_replace_claim_note() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let claim = zaphod(
        dir.path(),
        [
            "claim",
            "--agent",
            "codex",
            "--note",
            "implementing API handler",
        ],
    );
    assert_success(&claim);

    let output = zaphod(
        dir.path(),
        [
            "heartbeat",
            "--json",
            "--agent",
            "codex",
            "--note",
            "reviewing tests",
        ],
    );

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("heartbeat json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["status"], "refreshed");
    assert_eq!(report["claim"]["note"], "reviewing tests");

    let claims = zaphod(dir.path(), ["claims", "--json"]);
    assert_success(&claims);
    let report: serde_json::Value = serde_json::from_slice(&claims.stdout).expect("claims json");
    assert_eq!(report["claims"][0]["note"], "reviewing tests");
}

#[test]
fn heartbeat_json_can_clear_claim_note() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let claim = zaphod(
        dir.path(),
        [
            "claim",
            "--agent",
            "codex",
            "--note",
            "implementing API handler",
        ],
    );
    assert_success(&claim);

    let output = zaphod(
        dir.path(),
        ["heartbeat", "--json", "--agent", "codex", "--clear-note"],
    );

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("heartbeat json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["status"], "refreshed");
    assert!(report["claim"]["note"].is_null());

    let claims = zaphod(dir.path(), ["claims", "--json"]);
    assert_success(&claims);
    let report: serde_json::Value = serde_json::from_slice(&claims.stdout).expect("claims json");
    assert!(report["claims"][0]["note"].is_null());
}

#[test]
fn heartbeat_json_records_expected_branch_and_side() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let claim = zaphod(dir.path(), ["claim", "--agent", "codex"]);
    assert_success(&claim);

    let output = zaphod(
        dir.path(),
        [
            "heartbeat",
            "--json",
            "--agent",
            "codex",
            "--branch",
            "feature/api",
            "--side",
            "left",
        ],
    );

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("heartbeat json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["status"], "refreshed");
    assert_eq!(report["expectation"]["ok"], true);
    assert_eq!(report["expectation"]["expected_branch"], "feature/api");
    assert_eq!(report["expectation"]["expected_side"], "left");
    assert_eq!(report["expectation"]["current_side"], "left");
    assert_eq!(report["expectation"]["failures"], json!([]));
    assert_eq!(report["claim"]["agent"], "codex");
    assert_eq!(report["claim"]["branch"], "feature/api");
}

#[test]
fn claims_json_filters_by_agent_pair_and_branch() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let default_pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&default_pair);
    let api_pair = zaphod(dir.path(), ["pair", "main", "feature/api", "--name", "api"]);
    assert_success(&api_pair);
    let default_claim = zaphod(dir.path(), ["claim", "--agent", "codex"]);
    assert_success(&default_claim);
    let api_claim = zaphod(dir.path(), ["claim", "--agent", "codex", "--pair", "api"]);
    assert_success(&api_claim);

    let claims = zaphod(
        dir.path(),
        [
            "claims",
            "--json",
            "--agent",
            "codex",
            "--pair",
            "api",
            "--branch",
            "feature/api",
        ],
    );

    assert_success(&claims);
    let report: serde_json::Value = serde_json::from_slice(&claims.stdout).expect("claims json");
    assert_eq!(report["filters"]["agent"], "codex");
    assert_eq!(report["filters"]["pair"], "api");
    assert_eq!(report["filters"]["branch"], "feature/api");
    assert_eq!(report["claims"].as_array().expect("claims").len(), 1);
    assert_eq!(report["claims"][0]["agent"], "codex");
    assert_eq!(report["claims"][0]["pair"], "api");
    assert_eq!(report["claims"][0]["branch"], "feature/api");
}

#[test]
fn claims_json_filters_to_current_branch() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let api_claim = zaphod(dir.path(), ["claim", "--agent", "api-agent"]);
    assert_success(&api_claim);
    let switch = zaphod(dir.path(), ["switch"]);
    assert_success(&switch);
    assert_eq!(current_branch(dir.path()), "feature/ui");
    let ui_claim = zaphod(dir.path(), ["claim", "--agent", "ui-agent"]);
    assert_success(&ui_claim);
    let switch = zaphod(dir.path(), ["switch"]);
    assert_success(&switch);
    assert_eq!(current_branch(dir.path()), "feature/api");

    let claims = zaphod(dir.path(), ["claims", "--json", "--current"]);

    assert_success(&claims);
    let report: serde_json::Value = serde_json::from_slice(&claims.stdout).expect("claims json");
    assert_eq!(report["filters"]["current"], true);
    assert_eq!(report["filters"]["branch"], "feature/api");
    assert_eq!(report["claims"].as_array().expect("claims").len(), 1);
    assert_eq!(report["claims"][0]["agent"], "api-agent");
    assert_eq!(report["claims"][0]["branch"], "feature/api");
}

#[test]
fn claims_json_filters_to_target_branch_without_switching() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let api_claim = zaphod(dir.path(), ["claim", "--agent", "api-agent"]);
    assert_success(&api_claim);
    let switch = zaphod(dir.path(), ["switch"]);
    assert_success(&switch);
    let ui_claim = zaphod(dir.path(), ["claim", "--agent", "ui-agent"]);
    assert_success(&ui_claim);
    let switch_back = zaphod(dir.path(), ["switch"]);
    assert_success(&switch_back);

    let claims = zaphod(dir.path(), ["claims", "--json", "--target"]);

    assert_success(&claims);
    assert_eq!(current_branch(dir.path()), "feature/api");
    let report: serde_json::Value = serde_json::from_slice(&claims.stdout).expect("claims json");
    assert_eq!(report["filters"]["pair"], "default");
    assert_eq!(report["filters"]["branch"], "feature/ui");
    assert_eq!(report["filters"]["current"], false);
    assert_eq!(report["filters"]["target"], true);
    assert!(report["filters"]["side"].is_null());
    assert_eq!(report["claims"].as_array().expect("claims").len(), 1);
    assert_eq!(report["claims"][0]["agent"], "ui-agent");
    assert_eq!(report["claims"][0]["pair"], "default");
    assert_eq!(report["claims"][0]["branch"], "feature/ui");
}

#[test]
fn claims_json_filters_to_pair_side_without_switching() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let api_claim = zaphod(dir.path(), ["claim", "--agent", "api-agent"]);
    assert_success(&api_claim);
    let switch = zaphod(dir.path(), ["switch"]);
    assert_success(&switch);
    assert_eq!(current_branch(dir.path()), "feature/ui");
    let ui_claim = zaphod(dir.path(), ["claim", "--agent", "ui-agent"]);
    assert_success(&ui_claim);
    let switch = zaphod(dir.path(), ["switch"]);
    assert_success(&switch);
    assert_eq!(current_branch(dir.path()), "feature/api");

    let claims = zaphod(dir.path(), ["claims", "--json", "--side", "right"]);

    assert_success(&claims);
    assert_eq!(current_branch(dir.path()), "feature/api");
    let report: serde_json::Value = serde_json::from_slice(&claims.stdout).expect("claims json");
    assert_eq!(report["filters"]["pair"], "default");
    assert_eq!(report["filters"]["branch"], "feature/ui");
    assert_eq!(report["filters"]["current"], false);
    assert_eq!(report["filters"]["side"], "right");
    assert_eq!(report["claims"].as_array().expect("claims").len(), 1);
    assert_eq!(report["claims"][0]["agent"], "ui-agent");
    assert_eq!(report["claims"][0]["branch"], "feature/ui");
}

#[test]
fn claims_json_filters_conflicts_for_agent_on_current_branch() {
    let dir = TestDir::new("zaphod-cli-metadata");
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

[[claims]]
agent = "other-agent"
pair = "default"
branch = "feature/api"
created_at_unix = 2

[[claims]]
agent = "ui-agent"
pair = "default"
branch = "feature/ui"
created_at_unix = 3
"#,
    )
    .expect("write claims metadata");

    let claims = zaphod(
        dir.path(),
        ["claims", "--json", "--conflicts-for", "codex", "--current"],
    );

    assert_success(&claims);
    let report: serde_json::Value = serde_json::from_slice(&claims.stdout).expect("claims json");
    assert_eq!(report["filters"]["conflicts_for"], "codex");
    assert_eq!(report["filters"]["branch"], "feature/api");
    assert_eq!(report["filters"]["current"], true);
    assert_eq!(report["claims"].as_array().expect("claims").len(), 1);
    assert_eq!(report["claims"][0]["agent"], "other-agent");
    assert_eq!(report["claims"][0]["branch"], "feature/api");
}

#[test]
fn claims_json_filters_stale_claims() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let metadata_dir = dir.git_dir().join("zaphod");
    fs::create_dir_all(&metadata_dir).expect("create zaphod metadata directory");
    fs::write(
        metadata_dir.join("claims.toml"),
        r#"[[claims]]
agent = "old-agent"
pair = "default"
branch = "feature/api"
created_at_unix = 1

[[claims]]
agent = "fresh-agent"
pair = "default"
branch = "feature/api"
created_at_unix = 4102444800
"#,
    )
    .expect("write claims metadata");

    let claims = zaphod(dir.path(), ["claims", "--json", "--stale-after", "1d"]);

    assert_success(&claims);
    let report: serde_json::Value = serde_json::from_slice(&claims.stdout).expect("claims json");
    assert_eq!(report["filters"]["stale_after_seconds"], 86_400);
    assert_eq!(report["claims"].as_array().expect("claims").len(), 1);
    assert_eq!(report["claims"][0]["agent"], "old-agent");
}

#[test]
fn prune_claims_dry_runs_and_applies_stale_cleanup() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let metadata_dir = dir.git_dir().join("zaphod");
    fs::create_dir_all(&metadata_dir).expect("create zaphod metadata directory");
    let claims_path = metadata_dir.join("claims.toml");
    fs::write(
        &claims_path,
        r#"[[claims]]
agent = "old-agent"
pair = "default"
branch = "feature/api"
created_at_unix = 1

[[claims]]
agent = "fresh-agent"
pair = "default"
branch = "feature/api"
created_at_unix = 4102444800
"#,
    )
    .expect("write claims metadata");

    let dry_run = zaphod(
        dir.path(),
        [
            "prune-claims",
            "--json",
            "--pair",
            "default",
            "--stale-after",
            "1d",
        ],
    );

    assert_success(&dry_run);
    let report: serde_json::Value =
        serde_json::from_slice(&dry_run.stdout).expect("prune dry-run json");
    assert_eq!(report["applied"], false);
    assert_eq!(report["filters"]["stale_after_seconds"], 86_400);
    assert_eq!(report["pruned_claims"].as_array().expect("claims").len(), 1);
    assert_eq!(report["pruned_claims"][0]["agent"], "old-agent");
    assert_eq!(report["remaining_claim_count"], 2);
    let metadata = fs::read_to_string(&claims_path).expect("read claims metadata");
    assert!(metadata.contains("old-agent"));

    let apply = zaphod(
        dir.path(),
        [
            "prune-claims",
            "--json",
            "--pair",
            "default",
            "--stale-after",
            "1d",
            "--apply",
        ],
    );

    assert_success(&apply);
    let report: serde_json::Value =
        serde_json::from_slice(&apply.stdout).expect("prune apply json");
    assert_eq!(report["applied"], true);
    assert_eq!(report["pruned_claims"].as_array().expect("claims").len(), 1);
    assert_eq!(report["remaining_claim_count"], 1);

    let claims = zaphod(dir.path(), ["claims", "--json"]);
    assert_success(&claims);
    let report: serde_json::Value = serde_json::from_slice(&claims.stdout).expect("claims json");
    assert_eq!(report["claims"].as_array().expect("claims").len(), 1);
    assert_eq!(report["claims"][0]["agent"], "fresh-agent");
}

#[test]
fn prune_claims_current_filters_cleanup_to_current_branch() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let metadata_dir = dir.git_dir().join("zaphod");
    fs::create_dir_all(&metadata_dir).expect("create zaphod metadata directory");
    let claims_path = metadata_dir.join("claims.toml");
    fs::write(
        &claims_path,
        r#"[[claims]]
agent = "old-api-agent"
pair = "default"
branch = "feature/api"
created_at_unix = 1

[[claims]]
agent = "old-ui-agent"
pair = "default"
branch = "feature/ui"
created_at_unix = 1

[[claims]]
agent = "fresh-api-agent"
pair = "default"
branch = "feature/api"
created_at_unix = 4102444800
"#,
    )
    .expect("write claims metadata");

    let dry_run = zaphod(
        dir.path(),
        ["prune-claims", "--json", "--current", "--stale-after", "1d"],
    );

    assert_success(&dry_run);
    let report: serde_json::Value =
        serde_json::from_slice(&dry_run.stdout).expect("prune current dry-run json");
    assert_eq!(report["applied"], false);
    assert_eq!(report["filters"]["current"], true);
    assert_eq!(report["filters"]["branch"], "feature/api");
    assert_eq!(report["filters"]["stale_after_seconds"], 86_400);
    assert_eq!(report["pruned_claims"].as_array().expect("claims").len(), 1);
    assert_eq!(report["pruned_claims"][0]["agent"], "old-api-agent");
    assert_eq!(report["remaining_claim_count"], 3);
    let metadata = fs::read_to_string(&claims_path).expect("read claims metadata");
    assert!(metadata.contains("old-api-agent"));
    assert!(metadata.contains("old-ui-agent"));

    let apply = zaphod(
        dir.path(),
        [
            "prune-claims",
            "--json",
            "--current",
            "--stale-after",
            "1d",
            "--apply",
        ],
    );

    assert_success(&apply);
    let report: serde_json::Value =
        serde_json::from_slice(&apply.stdout).expect("prune current apply json");
    assert_eq!(report["applied"], true);
    assert_eq!(report["filters"]["current"], true);
    assert_eq!(report["filters"]["branch"], "feature/api");
    assert_eq!(report["pruned_claims"].as_array().expect("claims").len(), 1);
    assert_eq!(report["pruned_claims"][0]["agent"], "old-api-agent");
    assert_eq!(report["remaining_claim_count"], 2);

    let claims = zaphod(dir.path(), ["claims", "--json"]);
    assert_success(&claims);
    let report: serde_json::Value = serde_json::from_slice(&claims.stdout).expect("claims json");
    assert_eq!(report["claims"].as_array().expect("claims").len(), 2);
    assert_eq!(report["claims"][0]["agent"], "old-ui-agent");
    assert_eq!(report["claims"][1]["agent"], "fresh-api-agent");
}

#[test]
fn prune_claims_side_filters_cleanup_to_pair_side_without_switching() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let metadata_dir = dir.git_dir().join("zaphod");
    fs::create_dir_all(&metadata_dir).expect("create zaphod metadata directory");
    let claims_path = metadata_dir.join("claims.toml");
    fs::write(
        &claims_path,
        r#"[[claims]]
agent = "old-api-agent"
pair = "default"
branch = "feature/api"
created_at_unix = 1

[[claims]]
agent = "old-ui-agent"
pair = "default"
branch = "feature/ui"
created_at_unix = 1

[[claims]]
agent = "fresh-ui-agent"
pair = "default"
branch = "feature/ui"
created_at_unix = 4102444800
"#,
    )
    .expect("write claims metadata");
    assert_eq!(current_branch(dir.path()), "feature/api");

    let dry_run = zaphod(
        dir.path(),
        [
            "prune-claims",
            "--json",
            "--side",
            "right",
            "--stale-after",
            "1d",
        ],
    );

    assert_success(&dry_run);
    assert_eq!(current_branch(dir.path()), "feature/api");
    let report: serde_json::Value =
        serde_json::from_slice(&dry_run.stdout).expect("prune side dry-run json");
    assert_eq!(report["applied"], false);
    assert_eq!(report["filters"]["pair"], "default");
    assert_eq!(report["filters"]["branch"], "feature/ui");
    assert_eq!(report["filters"]["side"], "right");
    assert_eq!(report["filters"]["stale_after_seconds"], 86_400);
    assert_eq!(report["pruned_claims"].as_array().expect("claims").len(), 1);
    assert_eq!(report["pruned_claims"][0]["agent"], "old-ui-agent");
    assert_eq!(report["remaining_claim_count"], 3);
    let metadata = fs::read_to_string(&claims_path).expect("read claims metadata");
    assert!(metadata.contains("old-api-agent"));
    assert!(metadata.contains("old-ui-agent"));

    let apply = zaphod(
        dir.path(),
        [
            "prune-claims",
            "--json",
            "--side",
            "right",
            "--stale-after",
            "1d",
            "--apply",
        ],
    );

    assert_success(&apply);
    assert_eq!(current_branch(dir.path()), "feature/api");
    let report: serde_json::Value =
        serde_json::from_slice(&apply.stdout).expect("prune side apply json");
    assert_eq!(report["applied"], true);
    assert_eq!(report["filters"]["pair"], "default");
    assert_eq!(report["filters"]["branch"], "feature/ui");
    assert_eq!(report["filters"]["side"], "right");
    assert_eq!(report["pruned_claims"].as_array().expect("claims").len(), 1);
    assert_eq!(report["pruned_claims"][0]["agent"], "old-ui-agent");
    assert_eq!(report["remaining_claim_count"], 2);

    let claims = zaphod(dir.path(), ["claims", "--json"]);
    assert_success(&claims);
    let report: serde_json::Value = serde_json::from_slice(&claims.stdout).expect("claims json");
    assert_eq!(report["claims"].as_array().expect("claims").len(), 2);
    assert_eq!(report["claims"][0]["agent"], "old-api-agent");
    assert_eq!(report["claims"][1]["agent"], "fresh-ui-agent");
}

#[test]
fn prune_claims_target_filters_cleanup_to_target_branch_without_switching() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let metadata_dir = dir.git_dir().join("zaphod");
    fs::create_dir_all(&metadata_dir).expect("create zaphod metadata directory");
    let claims_path = metadata_dir.join("claims.toml");
    fs::write(
        &claims_path,
        r#"[[claims]]
agent = "old-api-agent"
pair = "default"
branch = "feature/api"
created_at_unix = 1

[[claims]]
agent = "old-ui-agent"
pair = "default"
branch = "feature/ui"
created_at_unix = 1

[[claims]]
agent = "fresh-ui-agent"
pair = "default"
branch = "feature/ui"
created_at_unix = 4102444800
"#,
    )
    .expect("write claims metadata");
    assert_eq!(current_branch(dir.path()), "feature/api");

    let dry_run = zaphod(
        dir.path(),
        ["prune-claims", "--json", "--target", "--stale-after", "1d"],
    );

    assert_success(&dry_run);
    assert_eq!(current_branch(dir.path()), "feature/api");
    let report: serde_json::Value =
        serde_json::from_slice(&dry_run.stdout).expect("prune target dry-run json");
    assert_eq!(report["applied"], false);
    assert_eq!(report["filters"]["pair"], "default");
    assert_eq!(report["filters"]["branch"], "feature/ui");
    assert_eq!(report["filters"]["current"], false);
    assert_eq!(report["filters"]["target"], true);
    assert!(report["filters"]["side"].is_null());
    assert_eq!(report["filters"]["stale_after_seconds"], 86_400);
    assert_eq!(report["pruned_claims"].as_array().expect("claims").len(), 1);
    assert_eq!(report["pruned_claims"][0]["agent"], "old-ui-agent");
    assert_eq!(report["remaining_claim_count"], 3);
    let metadata = fs::read_to_string(&claims_path).expect("read claims metadata");
    assert!(metadata.contains("old-api-agent"));
    assert!(metadata.contains("old-ui-agent"));

    let apply = zaphod(
        dir.path(),
        [
            "prune-claims",
            "--json",
            "--target",
            "--stale-after",
            "1d",
            "--apply",
        ],
    );

    assert_success(&apply);
    assert_eq!(current_branch(dir.path()), "feature/api");
    let report: serde_json::Value =
        serde_json::from_slice(&apply.stdout).expect("prune target apply json");
    assert_eq!(report["applied"], true);
    assert_eq!(report["filters"]["pair"], "default");
    assert_eq!(report["filters"]["branch"], "feature/ui");
    assert_eq!(report["filters"]["target"], true);
    assert_eq!(report["pruned_claims"].as_array().expect("claims").len(), 1);
    assert_eq!(report["pruned_claims"][0]["agent"], "old-ui-agent");
    assert_eq!(report["remaining_claim_count"], 2);

    let claims = zaphod(dir.path(), ["claims", "--json"]);
    assert_success(&claims);
    let report: serde_json::Value = serde_json::from_slice(&claims.stdout).expect("claims json");
    assert_eq!(report["claims"].as_array().expect("claims").len(), 2);
    assert_eq!(report["claims"][0]["agent"], "old-api-agent");
    assert_eq!(report["claims"][1]["agent"], "fresh-ui-agent");
}

#[test]
fn prune_claims_dry_runs_and_applies_orphaned_cleanup() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    git(dir.path(), ["branch", "-D", "feature/ui"]);
    let metadata_dir = dir.git_dir().join("zaphod");
    fs::create_dir_all(&metadata_dir).expect("create zaphod metadata directory");
    let claims_path = metadata_dir.join("claims.toml");
    fs::write(
        &claims_path,
        r#"[[claims]]
agent = "missing-pair"
pair = "removed"
branch = "feature/api"
created_at_unix = 1

[[claims]]
agent = "wrong-side"
pair = "default"
branch = "main"
created_at_unix = 2

[[claims]]
agent = "missing-branch"
pair = "default"
branch = "feature/ui"
created_at_unix = 3

[[claims]]
agent = "valid-agent"
pair = "default"
branch = "feature/api"
created_at_unix = 4
"#,
    )
    .expect("write claims metadata");

    let dry_run = zaphod(dir.path(), ["prune-claims", "--json", "--orphaned"]);

    assert_success(&dry_run);
    let report: serde_json::Value =
        serde_json::from_slice(&dry_run.stdout).expect("prune orphaned dry-run json");
    assert_eq!(report["applied"], false);
    assert_eq!(report["orphaned"], true);
    assert!(report["filters"]["stale_after_seconds"].is_null());
    assert_eq!(report["pruned_claims"].as_array().expect("claims").len(), 3);
    assert_eq!(
        report["pruned_claim_issues"]
            .as_array()
            .expect("claim issues")
            .len(),
        3
    );
    assert_eq!(report["pruned_claim_issues"][0]["reason"], "missing_pair");
    assert_eq!(
        report["pruned_claim_issues"][1]["reason"],
        "branch_not_in_pair"
    );
    assert_eq!(report["pruned_claim_issues"][2]["reason"], "missing_branch");
    assert_eq!(report["remaining_claim_count"], 4);
    let metadata = fs::read_to_string(&claims_path).expect("read claims metadata");
    assert!(metadata.contains("missing-pair"));

    let apply = zaphod(
        dir.path(),
        ["prune-claims", "--json", "--orphaned", "--apply"],
    );

    assert_success(&apply);
    let report: serde_json::Value =
        serde_json::from_slice(&apply.stdout).expect("prune orphaned apply json");
    assert_eq!(report["applied"], true);
    assert_eq!(report["pruned_claims"].as_array().expect("claims").len(), 3);
    assert_eq!(report["remaining_claim_count"], 1);

    let claims = zaphod(dir.path(), ["claims", "--json"]);
    assert_success(&claims);
    let report: serde_json::Value = serde_json::from_slice(&claims.stdout).expect("claims json");
    assert_eq!(report["claims"].as_array().expect("claims").len(), 1);
    assert_eq!(report["claims"][0]["agent"], "valid-agent");
    assert_eq!(report["claims"][0]["branch"], "feature/api");
}

#[test]
fn handoff_json_reports_pair_claims_and_agent_readiness() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let claim = zaphod(dir.path(), ["claim", "--agent", "codex"]);
    assert_success(&claim);

    let output = zaphod(dir.path(), ["handoff", "--json", "--agent", "other"]);

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("handoff json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["requested_pair"], "default");
    assert_eq!(report["requested_agent"], "other");
    assert_eq!(report["current_branch"], "feature/api");
    assert_eq!(report["worktree"], "clean");
    assert_eq!(report["git_state"], "ready");
    assert_eq!(report["pair"]["pair"], "default");
    assert_eq!(report["pair"]["current"], "feature/api");
    assert_eq!(report["pair"]["other"], "feature/ui");
    assert_eq!(report["pair"]["switch_allowed"], true);
    assert_eq!(report["claims"][0]["agent"], "codex");
    assert_eq!(report["claims"][0]["pair"], "default");
    assert_eq!(report["claim"]["requested_agent"], "other");
    assert_eq!(report["claim"]["claim_allowed"], false);
    assert_eq!(report["claim"]["metadata_lock"]["ok"], true);
    assert_eq!(report["claim"]["metadata_lock"]["locked"], false);
    assert!(report["claim"]["stale_after_seconds"].is_null());
    assert!(report["claim"]["conflict_stale"].is_null());
    assert_eq!(report["claim"]["conflict"]["agent"], "codex");
    assert_eq!(report["errors"], json!([]));
    assert!(report["generated_at_unix"].as_u64().is_some());
    assert!(report["repository_root"].as_str().is_some());
}

#[test]
fn handoff_json_requires_existing_claim_for_agent() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let claim = zaphod(dir.path(), ["claim", "--agent", "codex"]);
    assert_success(&claim);

    let output = zaphod(
        dir.path(),
        ["handoff", "--json", "--agent", "codex", "--require-claim"],
    );

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("handoff json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["requested_agent"], "codex");
    assert_eq!(report["claim"]["requested_agent"], "codex");
    assert_eq!(report["claim"]["claim_allowed"], true);
    assert_eq!(report["claim"]["claim_required"], true);
    assert_eq!(report["claim"]["claim_owned"], true);
    assert_eq!(report["claim"]["owned_claim"]["agent"], "codex");
    assert_eq!(report["claim"]["owned_claim"]["pair"], "default");
    assert_eq!(report["claim"]["owned_claim"]["branch"], "feature/api");
    assert!(report["claim"]["conflict"].is_null());
    assert_eq!(report["errors"], json!([]));
}

#[test]
fn handoff_json_records_expected_branch_and_side() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);

    let output = zaphod(
        dir.path(),
        [
            "handoff",
            "--json",
            "--branch",
            "feature/api",
            "--side",
            "left",
        ],
    );

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("handoff json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["current_branch"], "feature/api");
    assert_eq!(report["expectation"]["ok"], true);
    assert_eq!(report["expectation"]["expected_branch"], "feature/api");
    assert_eq!(report["expectation"]["expected_side"], "left");
    assert_eq!(report["expectation"]["current_side"], "left");
    assert_eq!(report["expectation"]["failures"], json!([]));
    assert_eq!(report["errors"], json!([]));
}

#[test]
fn handoff_json_marks_stale_claim_conflicts() {
    let dir = TestDir::new("zaphod-cli-metadata");
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
    .expect("write stale claim metadata");

    let output = zaphod(
        dir.path(),
        [
            "handoff",
            "--json",
            "--agent",
            "other",
            "--stale-after",
            "1d",
        ],
    );

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("handoff json");
    assert_eq!(report["claim"]["requested_agent"], "other");
    assert_eq!(report["claim"]["claim_allowed"], false);
    assert_eq!(report["claim"]["stale_after_seconds"], 86_400);
    assert_eq!(report["claim"]["conflict_stale"], true);
    assert_eq!(report["claim"]["conflict"]["agent"], "codex");
}

#[test]
fn unclaim_json_releases_current_pair_and_branch() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let claim = zaphod(dir.path(), ["claim", "--agent", "codex"]);
    assert_success(&claim);

    let output = zaphod(dir.path(), ["unclaim", "--json", "--agent", "codex"]);

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("unclaim json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["status"], "released");
    assert_eq!(report["agent"], "codex");
    assert_eq!(report["pair"], "default");
    assert_eq!(report["branch"], "feature/api");
    assert_eq!(report["claim"]["agent"], "codex");

    let claims = zaphod(dir.path(), ["claims", "--json"]);
    assert_success(&claims);
    let report: serde_json::Value = serde_json::from_slice(&claims.stdout).expect("claims json");
    assert_eq!(report["claims"], json!([]));
}

#[test]
fn unclaim_json_can_release_a_claim_from_another_branch() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let claim = zaphod(dir.path(), ["claim", "--agent", "codex"]);
    assert_success(&claim);
    git(dir.path(), ["switch", "feature/ui"]);

    let output = zaphod(
        dir.path(),
        [
            "unclaim",
            "--json",
            "--agent",
            "codex",
            "--branch",
            "feature/api",
        ],
    );

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("unclaim json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["status"], "released");
    assert_eq!(report["agent"], "codex");
    assert_eq!(report["pair"], "default");
    assert_eq!(report["branch"], "feature/api");
    assert_eq!(report["claim"]["branch"], "feature/api");
    assert_eq!(current_branch(dir.path()), "feature/ui");

    let claims = zaphod(dir.path(), ["claims", "--json"]);
    assert_success(&claims);
    let report: serde_json::Value = serde_json::from_slice(&claims.stdout).expect("claims json");
    assert_eq!(report["claims"], json!([]));
}

#[test]
fn unclaim_json_can_release_a_claim_by_pair_side() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let claim = zaphod(dir.path(), ["claim", "--agent", "codex"]);
    assert_success(&claim);
    git(dir.path(), ["switch", "feature/ui"]);

    let output = zaphod(
        dir.path(),
        ["unclaim", "--json", "--agent", "codex", "--side", "left"],
    );

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("unclaim json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["status"], "released");
    assert_eq!(report["agent"], "codex");
    assert_eq!(report["pair"], "default");
    assert_eq!(report["branch"], "feature/api");
    assert_eq!(report["claim"]["branch"], "feature/api");
    assert_eq!(current_branch(dir.path()), "feature/ui");

    let claims = zaphod(dir.path(), ["claims", "--json"]);
    assert_success(&claims);
    let report: serde_json::Value = serde_json::from_slice(&claims.stdout).expect("claims json");
    assert_eq!(report["claims"], json!([]));
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
fn switch_json_reports_successful_switch() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);

    let output = zaphod(dir.path(), ["switch", "--json"]);

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("switch json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["dry_run"], false);
    assert_eq!(report["switched"], true);
    assert_eq!(report["pair"], "default");
    assert_eq!(report["current"], "feature/api");
    assert_eq!(report["target"], "feature/ui");
    assert_eq!(report["worktree"], "clean");
    assert_eq!(report["git_state"], "ready");
    assert_eq!(report["refusal_reasons"], json!([]));
    assert!(report["repository_root"].as_str().is_some());
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
fn switch_json_dry_run_reports_target_without_switching() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);

    let output = zaphod(dir.path(), ["switch", "--dry-run", "--json"]);

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("switch json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["switched"], false);
    assert_eq!(report["current"], "feature/api");
    assert_eq!(report["target"], "feature/ui");
    assert_eq!(report["refusal_reasons"], json!([]));
    assert_eq!(current_branch(dir.path()), "feature/api");
}

#[test]
fn switch_json_requires_existing_target_claim_for_agent() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    git(dir.path(), ["switch", "feature/ui"]);
    let claim = zaphod(dir.path(), ["claim", "--agent", "codex"]);
    assert_success(&claim);
    git(dir.path(), ["switch", "feature/api"]);

    let output = zaphod(
        dir.path(),
        ["switch", "--json", "--agent", "codex", "--require-claim"],
    );

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("switch json");
    assert_eq!(report["ok"], true);
    assert_eq!(report["switched"], true);
    assert_eq!(report["current"], "feature/api");
    assert_eq!(report["target"], "feature/ui");
    assert_eq!(report["target_claim"]["requested_agent"], "codex");
    assert_eq!(report["target_claim"]["claim_allowed"], true);
    assert_eq!(report["target_claim"]["claim_required"], true);
    assert_eq!(report["target_claim"]["claim_owned"], true);
    assert_eq!(report["target_claim"]["owned_claim"]["agent"], "codex");
    assert_eq!(
        report["target_claim"]["owned_claim"]["branch"],
        "feature/ui"
    );
    assert!(report["target_claim"]["conflict"].is_null());
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
fn switch_json_reports_dirty_refusal() {
    let dir = TestDir::new("zaphod-cli-metadata");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    fs::write(dir.path().join("dirty.txt"), "local work\n").expect("write dirty file");

    let output = zaphod(dir.path(), ["switch", "--json"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(3));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("switch json");
    assert_eq!(report["ok"], false);
    assert_eq!(report["dry_run"], false);
    assert_eq!(report["switched"], false);
    assert_eq!(report["pair"], "default");
    assert_eq!(report["current"], "feature/api");
    assert_eq!(report["target"], "feature/ui");
    assert_eq!(report["worktree"], "dirty");
    assert_eq!(report["git_state"], "ready");
    assert_eq!(report["refusal_reasons"], json!(["dirty_worktree"]));
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
