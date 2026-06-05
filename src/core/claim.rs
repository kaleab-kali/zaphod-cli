use super::METADATA_SCHEMA_VERSION;
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl AgentClaim {
    pub fn new(
        agent: String,
        pair: String,
        branch: String,
        created_at_unix: u64,
    ) -> Result<Self, ClaimError> {
        Self::new_with_note(agent, pair, branch, created_at_unix, None)
    }

    pub fn new_with_note(
        agent: String,
        pair: String,
        branch: String,
        created_at_unix: u64,
        note: Option<String>,
    ) -> Result<Self, ClaimError> {
        validate_agent_name(&agent)?;
        validate_non_empty("pair", &pair)?;
        validate_non_empty("branch", &branch)?;
        if let Some(note) = &note {
            validate_claim_note(note)?;
        }

        Ok(Self {
            agent,
            pair,
            branch,
            created_at_unix,
            note,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentClaims {
    #[serde(default = "metadata_schema_version")]
    schema_version: u32,
    #[serde(default)]
    claims: Vec<AgentClaim>,
}

impl Default for AgentClaims {
    fn default() -> Self {
        Self {
            schema_version: METADATA_SCHEMA_VERSION,
            claims: Vec::new(),
        }
    }
}

impl AgentClaims {
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

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

fn metadata_schema_version() -> u32 {
    METADATA_SCHEMA_VERSION
}

#[derive(Debug, PartialEq, Eq)]
pub enum ClaimError {
    EmptyField { field: &'static str },
    InvalidAgent { agent: String },
    InvalidNote,
    NoteTooLong { max_chars: usize },
}

impl Display for ClaimError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(formatter, "{field} cannot be empty"),
            Self::InvalidAgent { agent } => write!(
                formatter,
                "agent name '{agent}' must contain only letters, numbers, '.', '_', or '-'"
            ),
            Self::InvalidNote => write!(formatter, "claim note cannot contain control characters"),
            Self::NoteTooLong { max_chars } => write!(
                formatter,
                "claim note cannot be longer than {max_chars} characters"
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

pub fn validate_claim_note(note: &str) -> Result<(), ClaimError> {
    const MAX_CLAIM_NOTE_CHARS: usize = 240;

    if note.trim().is_empty() {
        return Err(ClaimError::EmptyField { field: "note" });
    }
    if note.chars().count() > MAX_CLAIM_NOTE_CHARS {
        return Err(ClaimError::NoteTooLong {
            max_chars: MAX_CLAIM_NOTE_CHARS,
        });
    }
    if note.chars().any(char::is_control) {
        return Err(ClaimError::InvalidNote);
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
    use crate::core::METADATA_SCHEMA_VERSION;

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
    fn claim_can_store_a_note() {
        let claim = AgentClaim::new_with_note(
            "codex".to_owned(),
            "default".to_owned(),
            "feature/api".to_owned(),
            42,
            Some("implementing API handler".to_owned()),
        )
        .expect("valid claim note");

        assert_eq!(claim.note.as_deref(), Some("implementing API handler"));
    }

    #[test]
    fn claim_note_rejects_control_characters() {
        let error = AgentClaim::new_with_note(
            "codex".to_owned(),
            "default".to_owned(),
            "feature/api".to_owned(),
            42,
            Some("bad\nnote".to_owned()),
        )
        .expect_err("reject control character in note");

        assert_eq!(error, ClaimError::InvalidNote);
    }

    #[test]
    fn claim_note_rejects_blank_values() {
        let error = AgentClaim::new_with_note(
            "codex".to_owned(),
            "default".to_owned(),
            "feature/api".to_owned(),
            42,
            Some("   ".to_owned()),
        )
        .expect_err("reject blank note");

        assert_eq!(error, ClaimError::EmptyField { field: "note" });
    }

    #[test]
    fn claim_note_rejects_overlong_values() {
        let error = AgentClaim::new_with_note(
            "codex".to_owned(),
            "default".to_owned(),
            "feature/api".to_owned(),
            42,
            Some("a".repeat(241)),
        )
        .expect_err("reject overlong note");

        assert_eq!(error, ClaimError::NoteTooLong { max_chars: 240 });
    }

    #[test]
    fn collection_defaults_to_current_schema_version() {
        let claims = AgentClaims::default();

        assert_eq!(claims.schema_version(), METADATA_SCHEMA_VERSION);
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
