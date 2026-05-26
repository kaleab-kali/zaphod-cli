use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt::{self, Display, Formatter};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::string::FromUtf8Error;

#[derive(Debug)]
pub struct GitRepository {
    root: PathBuf,
    git_dir: PathBuf,
}

impl GitRepository {
    pub fn discover(start: impl AsRef<Path>) -> Result<Self, GitError> {
        let root = run_git(start.as_ref(), ["rev-parse", "--show-toplevel"])
            .map_err(|_| GitError::NotRepository)?;
        let git_dir = run_git(start.as_ref(), ["rev-parse", "--absolute-git-dir"])
            .map_err(|_| GitError::NotRepository)?;

        Ok(Self {
            root: PathBuf::from(root),
            git_dir: PathBuf::from(git_dir),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    pub fn current_branch(&self) -> Result<String, GitError> {
        let branch = run_git(&self.root, ["branch", "--show-current"])?;

        if branch.is_empty() {
            return Err(GitError::DetachedHead);
        }

        Ok(branch)
    }

    pub fn is_dirty(&self) -> Result<bool, GitError> {
        let status = run_git(&self.root, ["status", "--porcelain"])?;

        Ok(!status.is_empty())
    }

    pub fn is_merge_in_progress(&self) -> bool {
        self.git_dir.join("MERGE_HEAD").exists()
    }

    pub fn is_rebase_in_progress(&self) -> bool {
        self.git_dir.join("rebase-apply").exists() || self.git_dir.join("rebase-merge").exists()
    }

    pub fn branch_exists(&self, branch: &str) -> Result<bool, GitError> {
        let reference = format!("refs/heads/{branch}");
        let output = git_output(&self.root, ["rev-parse", "--verify", "--quiet", &reference])?;

        Ok(output.status.success())
    }
}

#[derive(Debug)]
pub enum GitError {
    CommandFailed { args: Vec<String>, stderr: String },
    DetachedHead,
    InvalidOutput { source: FromUtf8Error },
    LaunchFailed { source: std::io::Error },
    NotRepository,
}

impl Display for GitError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::CommandFailed { args, stderr } => {
                write!(formatter, "git command failed: git {}", args.join(" "))?;
                if !stderr.trim().is_empty() {
                    write!(formatter, ": {}", stderr.trim())?;
                }
                Ok(())
            }
            Self::DetachedHead => write!(formatter, "current Git HEAD is detached"),
            Self::InvalidOutput { .. } => write!(formatter, "git returned invalid UTF-8 output"),
            Self::LaunchFailed { .. } => write!(formatter, "failed to launch git"),
            Self::NotRepository => write!(formatter, "not inside a Git repository"),
        }
    }
}

impl Error for GitError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidOutput { source } => Some(source),
            Self::LaunchFailed { source } => Some(source),
            Self::CommandFailed { .. } | Self::DetachedHead | Self::NotRepository => None,
        }
    }
}

fn run_git<I, S>(working_dir: &Path, args: I) -> Result<String, GitError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args = collect_args(args);
    let output = git_output(working_dir, &args)?;

    if !output.status.success() {
        return Err(GitError::CommandFailed {
            args: display_args(&args),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    let stdout =
        String::from_utf8(output.stdout).map_err(|source| GitError::InvalidOutput { source })?;

    Ok(stdout.trim().to_owned())
}

fn git_output<I, S>(working_dir: &Path, args: I) -> Result<Output, GitError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new("git")
        .args(args)
        .current_dir(working_dir)
        .output()
        .map_err(|source| GitError::LaunchFailed { source })
}

fn collect_args<I, S>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    args.into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect()
}

fn display_args(args: &[OsString]) -> Vec<String> {
    args.iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{GitError, GitRepository};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Self {
            let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
            let path =
                std::env::temp_dir().join(format!("zaphod-test-{}-{id}", std::process::id()));
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
    fn discover_returns_repository_root() {
        let dir = TestDir::new();
        git(dir.path(), ["init", "--initial-branch=main"]);

        let nested = dir.path().join("nested");
        fs::create_dir(&nested).expect("create nested directory");

        let repo = GitRepository::discover(&nested).expect("discover repository");

        assert_eq!(repo.root(), dir.path());
    }

    #[test]
    fn discover_rejects_non_repository_directory() {
        let dir = TestDir::new();

        let error = GitRepository::discover(dir.path()).expect_err("reject non-repository");

        assert!(matches!(error, GitError::NotRepository));
    }

    #[test]
    fn current_branch_reads_unborn_branch() {
        let dir = TestDir::new();
        git(dir.path(), ["init", "--initial-branch=main"]);

        let repo = GitRepository::discover(dir.path()).expect("discover repository");

        assert_eq!(repo.current_branch().expect("current branch"), "main");
    }

    #[test]
    fn dirty_worktree_detects_untracked_file() {
        let dir = TestDir::new();
        git(dir.path(), ["init", "--initial-branch=main"]);
        fs::write(dir.path().join("README.md"), "# Test\n").expect("write file");

        let repo = GitRepository::discover(dir.path()).expect("discover repository");

        assert!(repo.is_dirty().expect("dirty status"));
    }

    #[test]
    fn clean_worktree_has_no_porcelain_status() {
        let dir = TestDir::new();
        git(dir.path(), ["init", "--initial-branch=main"]);
        git(dir.path(), ["config", "user.name", "Zaphod Test"]);
        git(
            dir.path(),
            ["config", "user.email", "zaphod@example.invalid"],
        );
        fs::write(dir.path().join("README.md"), "# Test\n").expect("write file");
        git(dir.path(), ["add", "README.md"]);
        git(dir.path(), ["commit", "-m", "test: initial commit"]);

        let repo = GitRepository::discover(dir.path()).expect("discover repository");

        assert!(!repo.is_dirty().expect("dirty status"));
        assert!(repo.branch_exists("main").expect("branch exists"));
        assert!(!repo.branch_exists("missing").expect("branch exists"));
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

        assert!(
            output.status.success(),
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
