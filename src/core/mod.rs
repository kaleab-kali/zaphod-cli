pub const METADATA_SCHEMA_VERSION: u32 = 1;

mod claim;
mod pair;
mod status;

pub use claim::{AgentClaim, AgentClaims, ClaimError, validate_agent_name};
pub use pair::{BranchPair, BranchPairs, PairError, validate_pair_name};
pub use status::{GitState, PairStatus, RefusalReason, StatusError, WorktreeStatus};
