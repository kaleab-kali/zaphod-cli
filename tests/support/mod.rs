#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

pub struct TestDir {
    path: PathBuf,
}

impl TestDir {
    pub fn new(prefix: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{id}", std::process::id()));
        fs::create_dir(&path).expect("create test directory");

        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn git_dir(&self) -> PathBuf {
        self.path.join(".git")
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub fn init_repo_with_pair_branches(path: &Path) {
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

pub fn zaphod<I, S>(working_dir: &Path, args: I) -> Output
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

pub fn git<I, S>(working_dir: &Path, args: I)
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = git_output(working_dir, args);

    assert_success(&output);
}

pub fn git_stdout<I, S>(working_dir: &Path, args: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = git_output(working_dir, args);
    assert_success(&output);

    String::from_utf8(output.stdout)
        .expect("git stdout utf8")
        .trim()
        .to_owned()
}

pub fn current_branch(working_dir: &Path) -> String {
    git_stdout(working_dir, ["branch", "--show-current"])
}

pub fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn assert_stdout_contains(output: &Output, expected: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains(expected),
        "stdout did not contain {expected:?}\nstdout:\n{stdout}"
    );
}

pub fn assert_stderr_contains(output: &Output, expected: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains(expected),
        "stderr did not contain {expected:?}\nstderr:\n{stderr}"
    );
}

fn git_output<I, S>(working_dir: &Path, args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    Command::new("git")
        .args(args)
        .current_dir(working_dir)
        .output()
        .expect("run git")
}
