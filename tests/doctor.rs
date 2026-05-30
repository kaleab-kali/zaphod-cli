mod support;

use serde_json::json;
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
    assert_stdout_contains(&output, "Claims: ok (0 claim(s), ");
}

#[test]
fn doctor_can_emit_json_for_healthy_repository() {
    let dir = TestDir::new("zaphod-cli-doctor");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);

    let output = zaphod(dir.path(), ["doctor", "--json"]);

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("doctor json");
    assert_eq!(report["healthy"], true);
    assert_eq!(report["git"]["ok"], true);
    assert!(
        report["git"]["version"]
            .as_str()
            .expect("git version")
            .starts_with("git version ")
    );
    assert_eq!(report["repository"]["ok"], true);
    assert_eq!(report["current_branch"]["branch"], "feature/api");
    assert_eq!(report["worktree"]["state"], "clean");
    assert_eq!(report["git_state"], "ready");
    assert_eq!(report["metadata"]["ok"], true);
    assert_eq!(report["metadata"]["pair_count"], 1);
    assert_eq!(report["claims"]["ok"], true);
    assert_eq!(report["claims"]["claim_count"], 0);
    assert!(report["claims"]["stale_after_seconds"].is_null());
    assert_eq!(
        report["metadata"]["pairs"],
        json!([
            {
                "name": "default",
                "left": "feature/api",
                "right": "feature/ui",
                "ok": true,
                "summary": "ok",
            }
        ])
    );
}

#[test]
fn doctor_json_reports_stale_claims() {
    let dir = TestDir::new("zaphod-cli-doctor");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
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

    let output = zaphod(dir.path(), ["doctor", "--json", "--stale-after", "1d"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(4));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("doctor json");
    assert_eq!(report["healthy"], false);
    assert_eq!(report["claims"]["ok"], true);
    assert_eq!(report["claims"]["claim_count"], 2);
    assert_eq!(report["claims"]["stale_after_seconds"], 86_400);
    assert_eq!(report["claims"]["stale_claim_count"], 1);
    assert_eq!(report["claims"]["stale_claims"][0]["agent"], "old-agent");
    assert_stderr_contains(&output, "doctor found problems");
}

#[test]
fn doctor_reports_claims_that_no_longer_match_pairs_or_branches() {
    let dir = TestDir::new("zaphod-cli-doctor");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    git(dir.path(), ["branch", "-D", "feature/ui"]);
    let metadata_dir = dir.git_dir().join("zaphod");
    fs::create_dir_all(&metadata_dir).expect("create zaphod metadata directory");
    fs::write(
        metadata_dir.join("claims.toml"),
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
"#,
    )
    .expect("write orphaned claims metadata");

    let output = zaphod(dir.path(), ["doctor"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(4));
    assert_stdout_contains(&output, "Claims: problems (3 claim(s), ");
    assert_stdout_contains(&output, "Claim issues: 3");
    assert_stdout_contains(
        &output,
        "- missing-pair: removed on feature/api [pair 'removed' is not configured]",
    );
    assert_stdout_contains(
        &output,
        "- wrong-side: default on main [branch 'main' is not part of pair 'default']",
    );
    assert_stdout_contains(
        &output,
        "- missing-branch: default on feature/ui [branch 'feature/ui' was not found]",
    );
    assert_stderr_contains(&output, "doctor found problems");

    let output = zaphod(dir.path(), ["doctor", "--json"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(4));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("doctor json");
    assert_eq!(report["healthy"], false);
    assert_eq!(report["claims"]["ok"], false);
    assert_eq!(report["claims"]["claim_count"], 3);
    assert_eq!(report["claims"]["claim_issue_count"], 3);
    assert_eq!(
        report["claims"]["claim_issues"][0]["reason"],
        "missing_pair"
    );
    assert_eq!(
        report["claims"]["claim_issues"][1]["reason"],
        "branch_not_in_pair"
    );
    assert_eq!(
        report["claims"]["claim_issues"][2]["reason"],
        "missing_branch"
    );
}

#[test]
fn doctor_reports_corrupt_claims_metadata() {
    let dir = TestDir::new("zaphod-cli-doctor");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    let metadata_dir = dir.git_dir().join("zaphod");
    fs::create_dir_all(&metadata_dir).expect("create zaphod metadata directory");
    fs::write(metadata_dir.join("claims.toml"), "claims = [").expect("write corrupt metadata");

    let output = zaphod(dir.path(), ["doctor"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(4));
    assert_stdout_contains(&output, "Claims: error (failed to parse metadata file");
    assert_stderr_contains(&output, "doctor found problems");
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
fn doctor_json_reports_missing_pair_branch() {
    let dir = TestDir::new("zaphod-cli-doctor");
    init_repo_with_pair_branches(dir.path());
    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    git(dir.path(), ["branch", "-D", "feature/ui"]);

    let output = zaphod(dir.path(), ["doctor", "--json"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(4));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("doctor json");
    assert_eq!(report["healthy"], false);
    assert_eq!(
        report["metadata"]["pairs"][0],
        json!({
            "name": "default",
            "left": "feature/api",
            "right": "feature/ui",
            "ok": false,
            "summary": "missing right branch: feature/ui",
        })
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
fn doctor_json_reports_directory_outside_git_repository() {
    let dir = TestDir::new("zaphod-cli-doctor");

    let output = zaphod(dir.path(), ["doctor", "--json"]);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(4));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).expect("doctor json");
    assert_eq!(report["healthy"], false);
    assert_eq!(report["git"]["ok"], true);
    assert_eq!(report["repository"]["ok"], false);
    assert_eq!(report["repository"]["error"], "not inside a Git repository");
    assert!(report["metadata"].is_null());
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
