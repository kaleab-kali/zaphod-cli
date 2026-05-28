use crate::cli::{Cli, CliCommand};
use crate::core::{
    BranchPair, GitState, PairError, PairStatus, RefusalReason, StatusError, WorktreeStatus,
};
use crate::git::{GitError, GitRepository};
use crate::metadata::{MetadataError, MetadataStore};
use clap::CommandFactory;
use clap_complete::Shell;
use serde::Serialize;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io;

pub fn run(cli: Cli) -> Result<(), AppError> {
    match cli.command {
        CliCommand::Pair { left, right, name } => pair_branches(name, left, right),
        CliCommand::List { json } => list_pairs(json),
        CliCommand::Unpair { name } => unpair_branches(&name),
        CliCommand::Rename { old, new } => rename_pair(&old, &new),
        CliCommand::Status { json, all, name } => {
            if all {
                show_all_statuses(json)
            } else {
                show_status(&name, json)
            }
        }
        CliCommand::Switch { name } => switch_branches(&name),
        CliCommand::Doctor => run_doctor(),
        CliCommand::Completions { shell } => generate_completions(shell),
    }
}

fn generate_completions(shell: Shell) -> Result<(), AppError> {
    let mut command = Cli::command();
    let mut stdout = io::stdout();

    clap_complete::generate(shell, &mut command, "zaphod", &mut stdout);

    Ok(())
}

fn pair_branches(name: String, left: String, right: String) -> Result<(), AppError> {
    let repository = GitRepository::discover(".")?;
    ensure_branch_name_is_valid(&repository, &left)?;
    ensure_branch_name_is_valid(&repository, &right)?;
    ensure_branch_exists(&repository, &left)?;
    ensure_branch_exists(&repository, &right)?;

    let pair = BranchPair::new(name, left, right)?;
    let store = MetadataStore::for_repository(&repository);
    let mut pairs = store.load()?;
    let replaced = pairs.upsert(pair.clone()).is_some();
    store.save(&pairs)?;

    if replaced {
        println!(
            "Updated pair '{}': {} <-> {}",
            pair.name, pair.left, pair.right
        );
    } else {
        println!("Paired '{}': {} <-> {}", pair.name, pair.left, pair.right);
    }

    Ok(())
}

fn list_pairs(json: bool) -> Result<(), AppError> {
    let repository = GitRepository::discover(".")?;
    let store = MetadataStore::for_repository(&repository);
    let pairs = store.load()?;

    if json {
        println!("{}", serde_json::to_string_pretty(pairs.pairs())?);
        return Ok(());
    }

    if pairs.is_empty() {
        println!("No branch pairs configured.");
        return Ok(());
    }

    for pair in pairs.pairs() {
        println!("{}: {} <-> {}", pair.name, pair.left, pair.right);
    }

    Ok(())
}

fn unpair_branches(name: &str) -> Result<(), AppError> {
    let repository = GitRepository::discover(".")?;
    let store = MetadataStore::for_repository(&repository);
    let mut pairs = store.load()?;
    let removed = pairs.remove(name).ok_or_else(|| AppError::PairNotFound {
        name: name.to_owned(),
    })?;
    store.save(&pairs)?;

    println!(
        "Removed pair '{}': {} <-> {}",
        removed.name, removed.left, removed.right
    );

    Ok(())
}

fn rename_pair(old_name: &str, new_name: &str) -> Result<(), AppError> {
    let repository = GitRepository::discover(".")?;
    let store = MetadataStore::for_repository(&repository);
    let mut pairs = store.load()?;

    if old_name != new_name && pairs.get(new_name).is_some() {
        return Err(AppError::PairAlreadyExists {
            name: new_name.to_owned(),
        });
    }

    let pair = pairs
        .remove(old_name)
        .ok_or_else(|| AppError::PairNotFound {
            name: old_name.to_owned(),
        })?;
    let renamed = BranchPair::new(new_name.to_owned(), pair.left, pair.right)?;
    pairs.upsert(renamed.clone());
    store.save(&pairs)?;

    println!(
        "Renamed pair '{}' to '{}': {} <-> {}",
        old_name, renamed.name, renamed.left, renamed.right
    );

    Ok(())
}

fn show_status(name: &str, json: bool) -> Result<(), AppError> {
    let context = load_status_context(name)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&context.status)?);
    } else {
        print_status(&context.status);
    }

    Ok(())
}

fn show_all_statuses(json: bool) -> Result<(), AppError> {
    let repository = GitRepository::discover(".")?;
    let store = MetadataStore::for_repository(&repository);
    let pairs = store.load()?;

    if pairs.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("No branch pairs configured.");
        }
        return Ok(());
    }

    let current = repository.current_branch()?;
    let is_dirty = repository.is_dirty()?;
    let is_merge_in_progress = repository.is_merge_in_progress();
    let is_rebase_in_progress = repository.is_rebase_in_progress();
    let reports = pairs
        .pairs()
        .iter()
        .map(|pair| {
            build_status_report(
                &repository,
                pair,
                &current,
                is_dirty,
                is_merge_in_progress,
                is_rebase_in_progress,
            )
        })
        .collect::<Result<Vec<_>, AppError>>()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&reports)?);
    } else {
        print_status_reports(&reports);
    }

    Ok(())
}

fn switch_branches(name: &str) -> Result<(), AppError> {
    let context = load_status_context(name)?;

    if !context.status.switch_allowed {
        return Err(AppError::SwitchRefused {
            reasons: context.status.refusal_reasons,
        });
    }

    context.repository.switch_branch(&context.status.other)?;

    println!(
        "Switched pair '{}': {} -> {}",
        context.status.pair, context.status.current, context.status.other
    );

    Ok(())
}

fn run_doctor() -> Result<(), AppError> {
    let mut healthy = true;

    match GitRepository::version() {
        Ok(version) => println!("Git: ok ({version})"),
        Err(error) => {
            println!("Git: error ({error})");
            return Err(AppError::DoctorFailed);
        }
    }

    let repository = match GitRepository::discover(".") {
        Ok(repository) => {
            println!("Repository: ok ({})", repository.root().display());
            repository
        }
        Err(error) => {
            println!("Repository: error ({error})");
            return Err(AppError::DoctorFailed);
        }
    };

    println!("Git directory: {}", repository.git_dir().display());

    match repository.current_branch() {
        Ok(branch) => println!("Current branch: {branch}"),
        Err(error) => {
            println!("Current branch: error ({error})");
            healthy = false;
        }
    }

    match repository.is_dirty() {
        Ok(is_dirty) => println!("Worktree: {}", if is_dirty { "dirty" } else { "clean" }),
        Err(error) => {
            println!("Worktree: error ({error})");
            healthy = false;
        }
    }

    println!(
        "Git state: {}",
        format_git_state(
            repository.is_merge_in_progress(),
            repository.is_rebase_in_progress()
        )
    );

    let store = MetadataStore::for_repository(&repository);
    match store.load() {
        Ok(pairs) => {
            println!(
                "Metadata: ok ({} pair(s), {})",
                pairs.pairs().len(),
                store.path().display()
            );

            if pairs.is_empty() {
                println!("Pairs: none configured");
            } else {
                println!("Pairs:");
                for pair in pairs.pairs() {
                    match diagnose_pair_branches(&repository, pair) {
                        Ok(summary) => {
                            println!(
                                "- {}: {} <-> {} [{}]",
                                pair.name, pair.left, pair.right, summary
                            );
                            if summary != "ok" {
                                healthy = false;
                            }
                        }
                        Err(error) => {
                            println!(
                                "- {}: {} <-> {} [error: {}]",
                                pair.name, pair.left, pair.right, error
                            );
                            healthy = false;
                        }
                    }
                }
            }
        }
        Err(error) => {
            println!("Metadata: error ({error})");
            healthy = false;
        }
    }

    if healthy {
        Ok(())
    } else {
        Err(AppError::DoctorFailed)
    }
}

fn load_status_context(name: &str) -> Result<StatusContext, AppError> {
    let repository = GitRepository::discover(".")?;
    let store = MetadataStore::for_repository(&repository);
    let pairs = store.load()?;
    let pair = pairs.get(name).ok_or_else(|| AppError::PairNotFound {
        name: name.to_owned(),
    })?;
    let current = repository.current_branch()?;
    let other = pair
        .other_branch(&current)
        .ok_or_else(|| StatusError::CurrentBranchNotPaired {
            pair: pair.name.clone(),
            branch: current.clone(),
        })?;
    let target_branch_exists = repository.branch_exists(other)?;
    let is_dirty = repository.is_dirty()?;

    let status = PairStatus::new(
        pair,
        current,
        is_dirty,
        repository.is_merge_in_progress(),
        repository.is_rebase_in_progress(),
        target_branch_exists,
    )
    .map_err(AppError::from)?;

    Ok(StatusContext { repository, status })
}

fn format_git_state(is_merge_in_progress: bool, is_rebase_in_progress: bool) -> &'static str {
    match (is_merge_in_progress, is_rebase_in_progress) {
        (false, false) => "ready",
        (true, false) => "merge in progress",
        (false, true) => "rebase in progress",
        (true, true) => "merge and rebase in progress",
    }
}

fn diagnose_pair_branches(
    repository: &GitRepository,
    pair: &BranchPair,
) -> Result<String, AppError> {
    let left_exists = repository.branch_exists(&pair.left)?;
    let right_exists = repository.branch_exists(&pair.right)?;

    match (left_exists, right_exists) {
        (true, true) => Ok("ok".to_owned()),
        (false, true) => Ok(format!("missing left branch: {}", pair.left)),
        (true, false) => Ok(format!("missing right branch: {}", pair.right)),
        (false, false) => Ok(format!(
            "missing both branches: {}, {}",
            pair.left, pair.right
        )),
    }
}

fn build_status_report(
    repository: &GitRepository,
    pair: &BranchPair,
    current: &str,
    is_dirty: bool,
    is_merge_in_progress: bool,
    is_rebase_in_progress: bool,
) -> Result<PairStatusReport, AppError> {
    let left_exists = repository.branch_exists(&pair.left)?;
    let right_exists = repository.branch_exists(&pair.right)?;
    let other = pair.other_branch(current).map(str::to_owned);
    let worktree = WorktreeStatus::from_dirty(is_dirty);
    let git_state = GitState::from_repository_state(is_merge_in_progress, is_rebase_in_progress);
    let mut refusal_reasons = Vec::new();

    if let Some(other) = &other {
        if is_dirty {
            refusal_reasons.push(StatusReportRefusalReason::DirtyWorktree);
        }
        if is_merge_in_progress {
            refusal_reasons.push(StatusReportRefusalReason::MergeInProgress);
        }
        if is_rebase_in_progress {
            refusal_reasons.push(StatusReportRefusalReason::RebaseInProgress);
        }

        let target_branch_exists = if other == &pair.left {
            left_exists
        } else {
            right_exists
        };
        if !target_branch_exists {
            refusal_reasons.push(StatusReportRefusalReason::TargetBranchMissing);
        }
    } else {
        refusal_reasons.push(StatusReportRefusalReason::CurrentBranchNotPaired);
    }

    Ok(PairStatusReport {
        pair: pair.name.clone(),
        left: pair.left.clone(),
        right: pair.right.clone(),
        current: current.to_owned(),
        active: other.is_some(),
        other,
        left_exists,
        right_exists,
        worktree,
        git_state,
        switch_allowed: refusal_reasons.is_empty(),
        refusal_reasons,
    })
}

fn print_status(status: &PairStatus) {
    println!("Pair: {}", status.pair);
    println!("Current: {}", status.current);
    println!("Other: {}", status.other);
    println!("Worktree: {}", status.worktree);
    println!("Git state: {}", status.git_state);

    if status.switch_allowed {
        println!("Switch: allowed");
    } else {
        let reasons = status
            .refusal_reasons
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        println!("Switch: refused ({reasons})");
    }
}

fn print_status_reports(reports: &[PairStatusReport]) {
    for (index, report) in reports.iter().enumerate() {
        if index > 0 {
            println!();
        }

        println!("Pair: {}", report.pair);
        println!("Branches: {} <-> {}", report.left, report.right);
        println!(
            "Branch health: {}",
            format_branch_health(
                report.left_exists,
                report.right_exists,
                &report.left,
                &report.right
            )
        );
        println!("Current: {}", report.current);

        if let Some(other) = &report.other {
            println!("Other: {other}");
        } else {
            println!("Other: unavailable");
        }

        println!("Worktree: {}", report.worktree);
        println!("Git state: {}", report.git_state);

        if report.switch_allowed {
            println!("Switch: allowed");
        } else {
            let reasons = report
                .refusal_reasons
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ");
            if report.active {
                println!("Switch: refused ({reasons})");
            } else {
                println!("Switch: not available ({reasons})");
            }
        }
    }
}

fn format_branch_health(left_exists: bool, right_exists: bool, left: &str, right: &str) -> String {
    match (left_exists, right_exists) {
        (true, true) => "ok".to_owned(),
        (false, true) => format!("missing left branch: {left}"),
        (true, false) => format!("missing right branch: {right}"),
        (false, false) => format!("missing both branches: {left}, {right}"),
    }
}

struct StatusContext {
    repository: GitRepository,
    status: PairStatus,
}

#[derive(Debug, Serialize)]
struct PairStatusReport {
    pair: String,
    left: String,
    right: String,
    current: String,
    active: bool,
    other: Option<String>,
    left_exists: bool,
    right_exists: bool,
    worktree: WorktreeStatus,
    git_state: GitState,
    switch_allowed: bool,
    refusal_reasons: Vec<StatusReportRefusalReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum StatusReportRefusalReason {
    CurrentBranchNotPaired,
    DirtyWorktree,
    MergeInProgress,
    RebaseInProgress,
    TargetBranchMissing,
}

impl Display for StatusReportRefusalReason {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentBranchNotPaired => {
                write!(formatter, "current branch is not part of pair")
            }
            Self::DirtyWorktree => write!(formatter, "worktree has uncommitted changes"),
            Self::MergeInProgress => write!(formatter, "merge is in progress"),
            Self::RebaseInProgress => write!(formatter, "rebase is in progress"),
            Self::TargetBranchMissing => write!(formatter, "target branch is missing"),
        }
    }
}

fn ensure_branch_exists(repository: &GitRepository, branch: &str) -> Result<(), AppError> {
    if repository.branch_exists(branch)? {
        return Ok(());
    }

    Err(AppError::BranchNotFound {
        branch: branch.to_owned(),
    })
}

fn ensure_branch_name_is_valid(repository: &GitRepository, branch: &str) -> Result<(), AppError> {
    if repository.branch_name_is_valid(branch)? {
        return Ok(());
    }

    Err(AppError::InvalidBranchName {
        branch: branch.to_owned(),
    })
}

#[derive(Debug)]
pub enum AppError {
    BranchNotFound { branch: String },
    DoctorFailed,
    Git { source: GitError },
    InvalidBranchName { branch: String },
    Metadata { source: MetadataError },
    Pair { source: PairError },
    PairAlreadyExists { name: String },
    PairNotFound { name: String },
    Serialize { source: serde_json::Error },
    Status { source: StatusError },
    SwitchRefused { reasons: Vec<RefusalReason> },
}

impl Display for AppError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::BranchNotFound { branch } => write!(formatter, "branch '{branch}' was not found"),
            Self::DoctorFailed => write!(formatter, "doctor found problems"),
            Self::Git { source } => Display::fmt(source, formatter),
            Self::InvalidBranchName { branch } => {
                write!(formatter, "branch name '{branch}' is invalid")
            }
            Self::Metadata { source } => Display::fmt(source, formatter),
            Self::Pair { source } => Display::fmt(source, formatter),
            Self::PairAlreadyExists { name } => write!(formatter, "pair '{name}' already exists"),
            Self::PairNotFound { name } => write!(formatter, "pair '{name}' was not found"),
            Self::Serialize { source } => Display::fmt(source, formatter),
            Self::Status { source } => Display::fmt(source, formatter),
            Self::SwitchRefused { reasons } => {
                let reasons = reasons
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ");
                write!(formatter, "refusing to switch: {reasons}")
            }
        }
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Git { source } => Some(source),
            Self::Metadata { source } => Some(source),
            Self::Pair { source } => Some(source),
            Self::Serialize { source } => Some(source),
            Self::Status { source } => Some(source),
            Self::BranchNotFound { .. }
            | Self::DoctorFailed
            | Self::InvalidBranchName { .. }
            | Self::PairAlreadyExists { .. }
            | Self::PairNotFound { .. }
            | Self::SwitchRefused { .. } => None,
        }
    }
}

impl AppError {
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::BranchNotFound { .. }
            | Self::InvalidBranchName { .. }
            | Self::Pair { .. }
            | Self::PairAlreadyExists { .. }
            | Self::PairNotFound { .. }
            | Self::Status { .. } => 2,
            Self::SwitchRefused { .. } => 3,
            Self::DoctorFailed => 4,
            Self::Git {
                source: GitError::DetachedHead | GitError::NotRepository,
            } => 2,
            Self::Git { .. } | Self::Metadata { .. } | Self::Serialize { .. } => 1,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::BranchNotFound { .. } => "branch_not_found",
            Self::DoctorFailed => "doctor_failed",
            Self::Git { source } => source.kind(),
            Self::InvalidBranchName { .. } => "invalid_branch_name",
            Self::Metadata { .. } => "metadata_error",
            Self::Pair { .. } => "pair_error",
            Self::PairAlreadyExists { .. } => "pair_already_exists",
            Self::PairNotFound { .. } => "pair_not_found",
            Self::Serialize { .. } => "serialize_error",
            Self::Status { .. } => "status_error",
            Self::SwitchRefused { .. } => "switch_refused",
        }
    }
}

impl From<GitError> for AppError {
    fn from(source: GitError) -> Self {
        Self::Git { source }
    }
}

impl From<MetadataError> for AppError {
    fn from(source: MetadataError) -> Self {
        Self::Metadata { source }
    }
}

impl From<PairError> for AppError {
    fn from(source: PairError) -> Self {
        Self::Pair { source }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(source: serde_json::Error) -> Self {
        Self::Serialize { source }
    }
}

impl From<StatusError> for AppError {
    fn from(source: StatusError) -> Self {
        Self::Status { source }
    }
}
