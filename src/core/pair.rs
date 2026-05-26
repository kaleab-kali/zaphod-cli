use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BranchPair {
    pub name: String,
    pub left: String,
    pub right: String,
}

impl BranchPair {
    pub fn new(name: String, left: String, right: String) -> Result<Self, PairError> {
        validate_pair_name(&name)?;
        validate_branch_name("left", &left)?;
        validate_branch_name("right", &right)?;

        if left == right {
            return Err(PairError::BranchesMustDiffer);
        }

        Ok(Self { name, left, right })
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BranchPairs {
    #[serde(default)]
    pairs: Vec<BranchPair>,
}

impl BranchPairs {
    pub fn pairs(&self) -> &[BranchPair] {
        &self.pairs
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    pub fn get(&self, name: &str) -> Option<&BranchPair> {
        self.pairs.iter().find(|pair| pair.name == name)
    }

    pub fn upsert(&mut self, pair: BranchPair) -> Option<BranchPair> {
        let previous = self.remove(&pair.name);
        self.pairs.push(pair);
        self.pairs.sort_by(|left, right| left.name.cmp(&right.name));

        previous
    }

    pub fn remove(&mut self, name: &str) -> Option<BranchPair> {
        let index = self.pairs.iter().position(|pair| pair.name == name)?;

        Some(self.pairs.remove(index))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PairError {
    BranchesMustDiffer,
    EmptyBranchName { field: &'static str },
    EmptyPairName,
    InvalidPairName { name: String },
}

impl Display for PairError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::BranchesMustDiffer => write!(formatter, "paired branches must be different"),
            Self::EmptyBranchName { field } => {
                write!(formatter, "{field} branch name cannot be empty")
            }
            Self::EmptyPairName => write!(formatter, "pair name cannot be empty"),
            Self::InvalidPairName { name } => write!(
                formatter,
                "pair name '{name}' must contain only letters, numbers, '.', '_', or '-'"
            ),
        }
    }
}

impl Error for PairError {}

fn validate_pair_name(name: &str) -> Result<(), PairError> {
    if name.is_empty() {
        return Err(PairError::EmptyPairName);
    }

    if !name
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-'))
    {
        return Err(PairError::InvalidPairName {
            name: name.to_owned(),
        });
    }

    Ok(())
}

fn validate_branch_name(field: &'static str, name: &str) -> Result<(), PairError> {
    if name.is_empty() {
        return Err(PairError::EmptyBranchName { field });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{BranchPair, BranchPairs, PairError};

    #[test]
    fn pair_requires_different_branches() {
        let error = BranchPair::new("default".to_owned(), "main".to_owned(), "main".to_owned())
            .expect_err("reject same branch");

        assert_eq!(error, PairError::BranchesMustDiffer);
    }

    #[test]
    fn pair_name_allows_simple_safe_characters() {
        let pair = BranchPair::new(
            "review_1.2".to_owned(),
            "feature/api".to_owned(),
            "feature/ui".to_owned(),
        )
        .expect("valid pair");

        assert_eq!(pair.name, "review_1.2");
    }

    #[test]
    fn collection_upserts_by_name() {
        let mut pairs = BranchPairs::default();
        let first = BranchPair::new(
            "default".to_owned(),
            "main".to_owned(),
            "feature/a".to_owned(),
        )
        .expect("valid pair");
        let second = BranchPair::new(
            "default".to_owned(),
            "main".to_owned(),
            "feature/b".to_owned(),
        )
        .expect("valid pair");

        assert!(pairs.upsert(first).is_none());
        let previous = pairs.upsert(second).expect("previous pair");

        assert_eq!(previous.right, "feature/a");
        assert_eq!(pairs.pairs().len(), 1);
        assert_eq!(pairs.get("default").expect("pair").right, "feature/b");
    }
}
