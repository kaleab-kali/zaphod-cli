use crate::cli::{Cli, CliCommand};
use crate::core::{BranchPair, PairError};
use crate::git::{GitError, GitRepository};
use crate::metadata::{MetadataError, MetadataStore};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

pub fn run(cli: Cli) -> Result<(), AppError> {
    match cli.command {
        CliCommand::Pair { left, right, name } => pair_branches(name, left, right),
        CliCommand::List => list_pairs(),
        CliCommand::Unpair { name } => unpair_branches(&name),
        CliCommand::Status { .. } => Err(AppError::NotImplemented { command: "status" }),
        CliCommand::Switch { .. } => Err(AppError::NotImplemented { command: "switch" }),
    }
}

fn pair_branches(name: String, left: String, right: String) -> Result<(), AppError> {
    let repository = GitRepository::discover(".")?;
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

fn list_pairs() -> Result<(), AppError> {
    let repository = GitRepository::discover(".")?;
    let store = MetadataStore::for_repository(&repository);
    let pairs = store.load()?;

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

fn ensure_branch_exists(repository: &GitRepository, branch: &str) -> Result<(), AppError> {
    if repository.branch_exists(branch)? {
        return Ok(());
    }

    Err(AppError::BranchNotFound {
        branch: branch.to_owned(),
    })
}

#[derive(Debug)]
pub enum AppError {
    BranchNotFound { branch: String },
    Git { source: GitError },
    Metadata { source: MetadataError },
    NotImplemented { command: &'static str },
    Pair { source: PairError },
    PairNotFound { name: String },
}

impl Display for AppError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::BranchNotFound { branch } => write!(formatter, "branch '{branch}' was not found"),
            Self::Git { source } => Display::fmt(source, formatter),
            Self::Metadata { source } => Display::fmt(source, formatter),
            Self::NotImplemented { command } => {
                write!(
                    formatter,
                    "zaphod {command} is planned but not implemented yet"
                )
            }
            Self::Pair { source } => Display::fmt(source, formatter),
            Self::PairNotFound { name } => write!(formatter, "pair '{name}' was not found"),
        }
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Git { source } => Some(source),
            Self::Metadata { source } => Some(source),
            Self::Pair { source } => Some(source),
            Self::BranchNotFound { .. }
            | Self::NotImplemented { .. }
            | Self::PairNotFound { .. } => None,
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
