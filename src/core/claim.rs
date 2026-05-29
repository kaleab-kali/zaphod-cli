use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentClaim {
    pub agent: String,
    pub pair: String,
    pub branch: String,
    pub created_at_unix: u64,
}

impl AgentClaim {
    pub fn new(
        agent: String,
        pair: String,
        branch: String,
        created_at_unix: u64,
    ) -> Result<Self, ClaimError> {
        validate_agent_name(&agent)?;
        validate_non_empty("pair", &pair)?;
        validate_non_empty("branch", &branch)?;

        Ok(Self {
            agent,
            pair,
            branch,
            created_at_unix,
        })
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentClaims {
    #[serde(default)]
    claims: Vec<AgentClaim>,
}

impl AgentClaims {
    pub fn claims(&self) -> &[AgentClaim] {
        &self.claims
    }

    pub fn is_empty(&self) -> bool {
        self.claims.is_empty()
    }

    pub fn conflict_for_scope(&self, agent: &str, pair: &str, branch: &str) -> Option<&AgentClaim> {
        self.claims
            .iter()
            .find(|claim| claim.pair == pair && claim.branch == branch && claim.agent != agent)
    }

    pub fn get_for_scope(&self, agent: &str, pair: &str, branch: &str) -> Option<&AgentClaim> {
        self.claims
            .iter()
            .find(|claim| claim.agent == agent && claim.pair == pair && claim.branch == branch)
    }

    pub fn upsert(&mut self, claim: AgentClaim) -> Option<AgentClaim> {
        let previous = self.remove(&claim.agent, &claim.pair, &claim.branch);
        self.claims.push(claim);
        self.claims.sort_by(|left, right| {
            left.pair
                .cmp(&right.pair)
                .then(left.branch.cmp(&right.branch))
                .then(left.agent.cmp(&right.agent))
        });

        previous
    }

    pub fn remove(&mut self, agent: &str, pair: &str, branch: &str) -> Option<AgentClaim> {
        let index = self.claims.iter().position(|claim| {
            claim.agent == agent && claim.pair == pair && claim.branch == branch
        })?;

        Some(self.claims.remove(index))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ClaimError {
    EmptyField { field: &'static str },
    InvalidAgent { agent: String },
}

impl Display for ClaimError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(formatter, "{field} cannot be empty"),
            Self::InvalidAgent { agent } => write!(
                formatter,
                "agent name '{agent}' must contain only letters, numbers, '.', '_', or '-'"
            ),
        }
    }
}

impl Error for ClaimError {}

pub fn validate_agent_name(agent: &str) -> Result<(), ClaimError> {
    if agent.is_empty() {
        return Err(ClaimError::EmptyField { field: "agent" });
    }

    if !agent
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-'))
    {
        return Err(ClaimError::InvalidAgent {
            agent: agent.to_owned(),
        });
    }

    Ok(())
}

fn validate_non_empty(field: &'static str, value: &str) -> Result<(), ClaimError> {
    if value.is_empty() {
        return Err(ClaimError::EmptyField { field });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AgentClaim, AgentClaims, ClaimError};

    #[test]
    fn agent_name_allows_simple_safe_characters() {
        let claim = AgentClaim::new(
            "codex_1.2".to_owned(),
            "default".to_owned(),
            "feature/api".to_owned(),
            42,
        )
        .expect("valid claim");

        assert_eq!(claim.agent, "codex_1.2");
    }

    #[test]
    fn agent_name_rejects_path_like_values() {
        let error = AgentClaim::new(
            "bad/name".to_owned(),
            "default".to_owned(),
            "feature/api".to_owned(),
            42,
        )
        .expect_err("reject invalid agent name");

        assert_eq!(
            error,
            ClaimError::InvalidAgent {
                agent: "bad/name".to_owned()
            }
        );
    }

    #[test]
    fn collection_detects_conflicting_scope() {
        let mut claims = AgentClaims::default();
        claims.upsert(
            AgentClaim::new(
                "codex".to_owned(),
                "default".to_owned(),
                "feature/api".to_owned(),
                42,
            )
            .expect("valid claim"),
        );

        let conflict = claims
            .conflict_for_scope("other", "default", "feature/api")
            .expect("conflict");

        assert_eq!(conflict.agent, "codex");
        assert!(
            claims
                .conflict_for_scope("codex", "default", "feature/api")
                .is_none()
        );
    }

    #[test]
    fn collection_upserts_by_agent_pair_and_branch() {
        let mut claims = AgentClaims::default();
        let first = AgentClaim::new(
            "codex".to_owned(),
            "default".to_owned(),
            "feature/api".to_owned(),
            42,
        )
        .expect("valid claim");
        let second = AgentClaim::new(
            "codex".to_owned(),
            "default".to_owned(),
            "feature/api".to_owned(),
            43,
        )
        .expect("valid claim");

        assert!(claims.upsert(first).is_none());
        let previous = claims.upsert(second).expect("previous claim");

        assert_eq!(previous.created_at_unix, 42);
        assert_eq!(claims.claims().len(), 1);
        assert_eq!(claims.claims()[0].created_at_unix, 43);
    }

    #[test]
    fn collection_removes_by_scope() {
        let mut claims = AgentClaims::default();
        claims.upsert(
            AgentClaim::new(
                "codex".to_owned(),
                "default".to_owned(),
                "feature/api".to_owned(),
                42,
            )
            .expect("valid claim"),
        );

        let removed = claims
            .remove("codex", "default", "feature/api")
            .expect("removed claim");

        assert_eq!(removed.agent, "codex");
        assert!(claims.is_empty());
    }
}
