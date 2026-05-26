mod support;

use std::fs;
use support::{
    TestDir, assert_stdout_contains, assert_success, current_branch, init_repo_with_pair_branches,
    zaphod,
};

#[test]
fn commands_discover_repository_from_nested_directory() {
    let dir = TestDir::new("zaphod-cli-nested");
    init_repo_with_pair_branches(dir.path());
    let nested = dir.path().join("packages").join("api");
    fs::create_dir_all(&nested).expect("create nested directory");

    let pair = zaphod(&nested, ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    assert_stdout_contains(&pair, "Paired 'default': feature/api <-> feature/ui");

    let metadata =
        fs::read_to_string(dir.git_dir().join("zaphod").join("pairs.toml")).expect("read metadata");
    assert!(metadata.contains("name = \"default\""));

    let status = zaphod(&nested, ["status"]);
    assert_success(&status);
    assert_stdout_contains(&status, "Pair: default");
    assert_stdout_contains(&status, "Current: feature/api");
    assert_stdout_contains(&status, "Other: feature/ui");

    let list = zaphod(&nested, ["list"]);
    assert_success(&list);
    assert_stdout_contains(&list, "default: feature/api <-> feature/ui");

    let switched = zaphod(&nested, ["switch"]);
    assert_success(&switched);
    assert_stdout_contains(
        &switched,
        "Switched pair 'default': feature/api -> feature/ui",
    );
    assert_eq!(current_branch(dir.path()), "feature/ui");
}
