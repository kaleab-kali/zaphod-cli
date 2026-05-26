use crate::core::BranchPair;
use serde::Serialize;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PairStatus {
    pub pair: String,
    pub current: String,
    pub other: String,
    pub worktree: WorktreeStatus,
    pub git_state: GitState,
    pub switch_allowed: bool,
    pub refusal_reasons: Vec<RefusalReason>,
}

impl PairStatus {
    pub fn new(
        pair: &BranchPair,
        current_branch: String,
        is_dirty: bool,
        is_merge_in_progress: bool,
        is_rebase_in_progress: bool,
        target_branch_exists: bool,
    ) -> Result<Self, StatusError> {
        let other = pair
            .other_branch(&current_branch)
            .ok_or_else(|| StatusError::CurrentBranchNotPaired {
                pair: pair.name.clone(),
                branch: current_branch.clone(),
            })?
            .to_owned();

        let worktree = WorktreeStatus::from_dirty(is_dirty);
        let git_state =
            GitState::from_repository_state(is_merge_in_progress, is_rebase_in_progress);
        let mut refusal_reasons = Vec::new();

        if is_dirty {
            refusal_reasons.push(RefusalReason::DirtyWorktree);
        }
        if is_merge_in_progress {
            refusal_reasons.push(RefusalReason::MergeInProgress);
        }
        if is_rebase_in_progress {
            refusal_reasons.push(RefusalReason::RebaseInProgress);
        }
        if !target_branch_exists {
            refusal_reasons.push(RefusalReason::TargetBranchMissing);
        }

        Ok(Self {
            pair: pair.name.clone(),
            current: current_branch,
            other,
            worktree,
            git_state,
            switch_allowed: refusal_reasons.is_empty(),
            refusal_reasons,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeStatus {
    Clean,
    Dirty,
}

impl WorktreeStatus {
    fn from_dirty(is_dirty: bool) -> Self {
        if is_dirty { Self::Dirty } else { Self::Clean }
    }
}

impl Display for WorktreeStatus {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clean => write!(formatter, "clean"),
            Self::Dirty => write!(formatter, "dirty"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GitState {
    Ready,
    MergeInProgress,
    RebaseInProgress,
    MergeAndRebaseInProgress,
}

impl GitState {
    fn from_repository_state(is_merge_in_progress: bool, is_rebase_in_progress: bool) -> Self {
        match (is_merge_in_progress, is_rebase_in_progress) {
            (false, false) => Self::Ready,
            (true, false) => Self::MergeInProgress,
            (false, true) => Self::RebaseInProgress,
            (true, true) => Self::MergeAndRebaseInProgress,
        }
    }
}

impl Display for GitState {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready => write!(formatter, "ready"),
            Self::MergeInProgress => write!(formatter, "merge in progress"),
            Self::RebaseInProgress => write!(formatter, "rebase in progress"),
            Self::MergeAndRebaseInProgress => write!(formatter, "merge and rebase in progress"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalReason {
    DirtyWorktree,
    MergeInProgress,
    RebaseInProgress,
    TargetBranchMissing,
}

impl Display for RefusalReason {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirtyWorktree => write!(formatter, "worktree has uncommitted changes"),
            Self::MergeInProgress => write!(formatter, "merge is in progress"),
            Self::RebaseInProgress => write!(formatter, "rebase is in progress"),
            Self::TargetBranchMissing => write!(formatter, "target branch is missing"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum StatusError {
    CurrentBranchNotPaired { pair: String, branch: String },
}

impl Display for StatusError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentBranchNotPaired { pair, branch } => {
                write!(
                    formatter,
                    "current branch '{branch}' is not part of pair '{pair}'"
                )
            }
        }
    }
}

impl Error for StatusError {}

#[cfg(test)]
mod tests {
    use super::{GitState, PairStatus, RefusalReason, StatusError, WorktreeStatus};
    use crate::core::BranchPair;

    #[test]
    fn clean_pair_status_allows_switching() {
        let pair = pair();

        let status = PairStatus::new(&pair, "feature/api".to_owned(), false, false, false, true)
            .expect("status");

        assert_eq!(status.other, "feature/ui");
        assert_eq!(status.worktree, WorktreeStatus::Clean);
        assert_eq!(status.git_state, GitState::Ready);
        assert!(status.switch_allowed);
        assert!(status.refusal_reasons.is_empty());
    }

    #[test]
    fn dirty_pair_status_refuses_switching() {
        let pair = pair();

        let status = PairStatus::new(&pair, "feature/api".to_owned(), true, false, false, true)
            .expect("status");

        assert!(!status.switch_allowed);
        assert_eq!(status.refusal_reasons, vec![RefusalReason::DirtyWorktree]);
    }

    #[test]
    fn status_rejects_unpaired_current_branch() {
        let pair = pair();

        let error = PairStatus::new(&pair, "main".to_owned(), false, false, false, true)
            .expect_err("reject unpaired branch");

        assert_eq!(
            error,
            StatusError::CurrentBranchNotPaired {
                pair: "default".to_owned(),
                branch: "main".to_owned(),
            }
        );
    }

    #[test]
    fn missing_target_branch_refuses_switching() {
        let pair = pair();

        let status = PairStatus::new(&pair, "feature/api".to_owned(), false, false, false, false)
            .expect("status");

        assert!(!status.switch_allowed);
        assert_eq!(
            status.refusal_reasons,
            vec![RefusalReason::TargetBranchMissing]
        );
    }

    fn pair() -> BranchPair {
        BranchPair::new(
            "default".to_owned(),
            "feature/api".to_owned(),
            "feature/ui".to_owned(),
        )
        .expect("valid pair")
    }
}
