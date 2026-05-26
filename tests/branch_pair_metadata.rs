use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("zaphod-cli-metadata-{}-{id}", std::process::id()));
        fs::create_dir(&path).expect("create test directory");

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn pair_list_and_unpair_update_repo_metadata() {
    let dir = TestDir::new();
    init_repo_with_pair_branches(dir.path());

    let pair = zaphod(dir.path(), ["pair", "feature/api", "feature/ui"]);
    assert_success(&pair);
    assert_stdout_contains(&pair, "Paired 'default': feature/api <-> feature/ui");

    let metadata = fs::read_to_string(dir.path().join(".git").join("zaphod").join("pairs.toml"))
        .expect("read metadata");
    assert!(metadata.contains("name = \"default\""));
    assert!(metadata.contains("left = \"feature/api\""));
    assert!(metadata.contains("right = \"feature/ui\""));

    let list = zaphod(dir.path(), ["list"]);
    assert_success(&list);
    assert_stdout_contains(&list, "default: feature/api <-> feature/ui");

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
fn pair_rejects_missing_branch() {
    let dir = TestDir::new();
    init_repo_with_pair_branches(dir.path());

    let output = zaphod(dir.path(), ["pair", "feature/api", "missing"]);

    assert!(!output.status.success());
    assert_stderr_contains(&output, "branch 'missing' was not found");
}

fn init_repo_with_pair_branches(path: &Path) {
    git(path, ["init", "--initial-branch=main"]);
    git(path, ["config", "user.name", "Zaphod Test"]);
    git(path, ["config", "user.email", "zaphod@example.invalid"]);
    fs::write(path.join("README.md"), "# Test\n").expect("write README");
    git(path, ["add", "README.md"]);
    git(path, ["commit", "-m", "test: initial commit"]);
    git(path, ["switch", "-c", "feature/api"]);
    git(path, ["switch", "main"]);
    git(path, ["switch", "-c", "feature/ui"]);
    git(path, ["switch", "feature/api"]);
}

fn zaphod<I, S>(working_dir: &Path, args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    Command::new(env!("CARGO_BIN_EXE_zaphod"))
        .args(args)
        .current_dir(working_dir)
        .output()
        .expect("run zaphod")
}

fn git<I, S>(working_dir: &Path, args: I)
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = Command::new("git")
        .args(args)
        .current_dir(working_dir)
        .output()
        .expect("run git");

    assert_success(&output);
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_stdout_contains(output: &Output, expected: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains(expected),
        "stdout did not contain {expected:?}\nstdout:\n{stdout}"
    );
}

fn assert_stderr_contains(output: &Output, expected: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains(expected),
        "stderr did not contain {expected:?}\nstderr:\n{stderr}"
    );
}
