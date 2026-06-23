use crate::cli::{Cli, CliCommand, PairSide};
use crate::core::{
    AgentClaim, AgentClaims, BranchPair, BranchPairs, ClaimError, GitState, PairError, PairStatus,
    RefusalReason, StatusError, WorktreeStatus, validate_agent_name, validate_claim_note,
    validate_pair_name,
};
use crate::git::{GitError, GitRepository};
use crate::metadata::{ClaimStore, MetadataError, MetadataStore};
use clap::CommandFactory;
use clap_complete::Shell;
use serde::Serialize;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io;
use std::path::Path;
use std::time::{SystemTime, SystemTimeError, UNIX_EPOCH};

pub fn run(cli: Cli) -> Result<(), AppError> {
    match cli.command {
        CliCommand::Pair {
            left,
            right,
            name,
            json,
        } => pair_branches(name, left, right, json),
        CliCommand::Init { other, name, json } => init_pair(name, other, json),
        CliCommand::List { json } => list_pairs(json),
        CliCommand::Unpair { json, name } => unpair_branches(&name, json),
        CliCommand::Rename { json, old, new } => rename_pair(&old, &new, json),
        CliCommand::Status { json, all, name } => {
            if all {
                show_all_statuses(json)
            } else {
                show_status(&name, json)
            }
        }
        CliCommand::Switch {
            json,
            dry_run,
            agent,
            require_claim,
            name,
        } => switch_branches(&name, json, dry_run, agent, require_claim),
        CliCommand::Preflight {
            json,
            name,
            branch,
            side,
            agent,
            require_claim,
            stale_after,
        } => run_preflight(&name, json, branch, side, agent, require_claim, stale_after),
        CliCommand::Assert {
            json,
            pair,
            branch,
            side,
            agent,
            require_claim,
        } => assert_repository_state(json, pair, branch, side, agent, require_claim),
        CliCommand::Claim {
            json,
            agent,
            pair,
            branch,
            side,
            note,
            clear_note,
            stale_after,
        } => claim_current_scope(ClaimScopeOptions {
            json,
            agent,
            pair_name: pair,
            expected_branch: branch,
            expected_side: side,
            note,
            clear_note,
            stale_after,
        }),
        CliCommand::Heartbeat {
            json,
            agent,
            pair,
            branch,
            side,
            note,
            clear_note,
            stale_after,
        } => heartbeat_claim(ClaimScopeOptions {
            json,
            agent,
            pair_name: pair,
            expected_branch: branch,
            expected_side: side,
            note,
            clear_note,
            stale_after,
        }),
        CliCommand::Claims {
            json,
            agent,
            conflicts_for,
            pair,
            branch,
            current,
            target,
            side,
            stale_after,
        } => list_claims(ListClaimsOptions {
            json,
            agent,
            conflicts_for,
            pair,
            branch,
            current,
            target,
            side,
            stale_after,
        }),
        CliCommand::PruneClaims {
            json,
            agent,
            pair,
            branch,
            current,
            target,
            side,
            stale_after,
            orphaned,
            apply,
        } => prune_claims(PruneClaimsOptions {
            json,
            agent,
            pair,
            branch,
            current,
            target,
            side,
            stale_after,
            orphaned,
            apply,
        }),
        CliCommand::Unclaim {
            json,
            agent,
            pair,
            branch,
            side,
        } => unclaim_current_scope(json, agent, pair, branch, side),
        CliCommand::Handoff {
            json,
            name,
            branch,
            side,
            agent,
            require_claim,
            stale_after,
        } => show_handoff(json, name, branch, side, agent, require_claim, stale_after),
        CliCommand::Doctor { json, stale_after } => run_doctor(json, stale_after),
        CliCommand::Completions { shell } => generate_completions(shell),
    }
}

fn generate_completions(shell: Shell) -> Result<(), AppError> {
    let mut command = Cli::command();
    let mut stdout = io::stdout();

    clap_complete::generate(shell, &mut command, "zaphod", &mut stdout);

    Ok(())
}

fn pair_branches(name: String, left: String, right: String, json: bool) -> Result<(), AppError> {
    let repository = GitRepository::discover(".")?;
    ensure_branch_name_is_valid(&repository, &left)?;
    ensure_branch_name_is_valid(&repository, &right)?;
    ensure_branch_exists(&repository, &left)?;
    ensure_branch_exists(&repository, &right)?;

    let pair = BranchPair::new(name, left, right)?;
    let store = MetadataStore::for_repository(&repository);
    let previous_pair = save_pair(&store, pair.clone())?;
    let action = if previous_pair.is_some() {
        "updated"
    } else {
        "created"
    };
    let report = PairMutationReport::new(action, &repository, &store, pair.clone(), previous_pair);

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if report.previous_pair.is_some() {
        println!(
            "Updated pair '{}': {} <-> {}",
            pair.name, pair.left, pair.right
        );
    } else {
        println!("Paired '{}': {} <-> {}", pair.name, pair.left, pair.right);
    }

    Ok(())
}

fn init_pair(name: String, other: String, json: bool) -> Result<(), AppError> {
    let repository = GitRepository::discover(".")?;
    let current = repository.current_branch()?;
    ensure_branch_name_is_valid(&repository, &other)?;
    ensure_branch_exists(&repository, &current)?;
    ensure_branch_exists(&repository, &other)?;

    let pair = BranchPair::new(name, current, other)?;
    let store = MetadataStore::for_repository(&repository);
    let previous_pair = save_pair(&store, pair.clone())?;
    let action = if previous_pair.is_some() {
        "updated"
    } else {
        "initialized"
    };
    let report = PairMutationReport::new(action, &repository, &store, pair.clone(), previous_pair);

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if report.previous_pair.is_some() {
        println!(
            "Updated pair '{}': {} <-> {}",
            pair.name, pair.left, pair.right
        );
    } else {
        println!(
            "Initialized pair '{}': {} <-> {}",
            pair.name, pair.left, pair.right
        );
    }

    Ok(())
}

fn save_pair(store: &MetadataStore, pair: BranchPair) -> Result<Option<BranchPair>, AppError> {
    let _lock = store.lock()?;
    let mut pairs = store.load()?;
    let previous_pair = pairs.upsert(pair);
    store.save(&pairs)?;

    Ok(previous_pair)
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

fn unpair_branches(name: &str, json: bool) -> Result<(), AppError> {
    let repository = GitRepository::discover(".")?;
    let store = MetadataStore::for_repository(&repository);
    let _lock = store.lock()?;
    let mut pairs = store.load()?;
    let removed = pairs.remove(name).ok_or_else(|| AppError::PairNotFound {
        name: name.to_owned(),
    })?;
    store.save(&pairs)?;
    let report = PairMutationReport::new("removed", &repository, &store, removed.clone(), None);

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "Removed pair '{}': {} <-> {}",
            removed.name, removed.left, removed.right
        );
    }

    Ok(())
}

fn rename_pair(old_name: &str, new_name: &str, json: bool) -> Result<(), AppError> {
    let repository = GitRepository::discover(".")?;
    let store = MetadataStore::for_repository(&repository);
    let _lock = store.lock()?;
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
    let renamed = BranchPair::new(new_name.to_owned(), pair.left.clone(), pair.right.clone())?;
    pairs.upsert(renamed.clone());
    store.save(&pairs)?;
    let report = PairMutationReport::new(
        "renamed",
        &repository,
        &store,
        renamed.clone(),
        Some(pair.clone()),
    );

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "Renamed pair '{}' to '{}': {} <-> {}",
            old_name, renamed.name, renamed.left, renamed.right
        );
    }

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

fn switch_branches(
    name: &str,
    json: bool,
    dry_run: bool,
    agent: Option<String>,
    require_claim: bool,
) -> Result<(), AppError> {
    if let Some(agent) = &agent {
        validate_agent_name(agent)?;
    }
    let context = load_status_context(name)?;
    let mut report = SwitchReport::from_status_context(&context, dry_run);

    if !context.status.switch_allowed {
        if json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        return Err(AppError::SwitchRefused {
            reasons: context.status.refusal_reasons,
        });
    }

    if let Some(agent) = agent {
        let claim_report = build_claim_report_for_scope(
            &context.repository,
            &context.status.pair,
            &context.status.other,
            agent,
            require_claim,
            None,
        )?;
        let conflict = claim_report.conflict.clone();
        let claim_blocked = if conflict.is_none() && !claim_report.metadata_lock.ok {
            Some(AppError::ClaimBlocked {
                agent: claim_report.requested_agent.clone(),
                pair: context.status.pair.clone(),
                branch: context.status.other.clone(),
                reason: metadata_lock_block_reason(&claim_report.metadata_lock),
            })
        } else {
            None
        };
        let claim_required = if require_claim && conflict.is_none() && !claim_report.claim_owned {
            Some(AppError::ClaimRequired {
                agent: claim_report.requested_agent.clone(),
                pair: context.status.pair.clone(),
                branch: context.status.other.clone(),
            })
        } else {
            None
        };
        report.ok =
            report.ok && claim_report.claim_allowed && (!require_claim || claim_report.claim_owned);
        report.target_claim = Some(claim_report);

        if let Some(conflict) = conflict {
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
            return Err(AppError::ClaimConflict {
                agent: conflict.agent,
                pair: conflict.pair,
                branch: conflict.branch,
            });
        }
        if let Some(error) = claim_blocked {
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
            return Err(error);
        }
        if let Some(error) = claim_required {
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
            return Err(error);
        }
    }

    if dry_run {
        if json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            println!(
                "Would switch pair '{}': {} -> {}",
                context.status.pair, context.status.current, context.status.other
            );
            print_switch_target_claim(&report);
        }
        return Ok(());
    }

    context.repository.switch_branch(&context.status.other)?;
    report.ok = true;
    report.switched = true;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "Switched pair '{}': {} -> {}",
            context.status.pair, context.status.current, context.status.other
        );
        print_switch_target_claim(&report);
    }

    Ok(())
}

fn run_preflight(
    name: &str,
    json: bool,
    expected_branch: Option<String>,
    expected_side: Option<PairSide>,
    agent: Option<String>,
    require_claim: bool,
    stale_after: Option<String>,
) -> Result<(), AppError> {
    if let Some(agent) = &agent {
        validate_agent_name(agent)?;
    }
    let stale_after_seconds = stale_after
        .as_deref()
        .map(parse_duration_seconds)
        .transpose()?;

    match load_status_context(name) {
        Ok(context) => {
            let mut report = PreflightReport::from_status_context(&context);
            let expectation_error = if let Some(expectation) =
                build_branch_expectation_report(&context, expected_branch, expected_side)
            {
                report.ready = report.ready && expectation.ok;
                let error = (!expectation.ok).then(|| AppError::AssertFailed {
                    failures: expectation.failures.clone(),
                });
                report.expectation = Some(expectation);
                error
            } else {
                None
            };
            let mut claim_blocked = None;
            let mut claim_required = None;
            let claim_conflict = if let Some(agent) = agent {
                let (claim_report, target_claim_report) = build_preflight_claim_reports(
                    &context,
                    agent,
                    require_claim,
                    stale_after_seconds,
                )?;
                let conflict = claim_report.conflict.clone();
                if conflict.is_none() && !claim_report.metadata_lock.ok {
                    claim_blocked = Some(AppError::ClaimBlocked {
                        agent: claim_report.requested_agent.clone(),
                        pair: context.status.pair.clone(),
                        branch: context.status.current.clone(),
                        reason: metadata_lock_block_reason(&claim_report.metadata_lock),
                    });
                }
                if require_claim && conflict.is_none() && !claim_report.claim_owned {
                    claim_required = Some(AppError::ClaimRequired {
                        agent: claim_report.requested_agent.clone(),
                        pair: context.status.pair.clone(),
                        branch: context.status.current.clone(),
                    });
                }
                report.ready = report.ready
                    && claim_report.claim_allowed
                    && (!require_claim || claim_report.claim_owned);
                report.claim = Some(claim_report);
                report.target_claim = Some(target_claim_report);
                conflict
            } else {
                None
            };

            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_preflight_report(&report);
            }

            if let Some(conflict) = claim_conflict {
                Err(AppError::ClaimConflict {
                    agent: conflict.agent,
                    pair: conflict.pair,
                    branch: conflict.branch,
                })
            } else if let Some(error) = claim_blocked {
                Err(error)
            } else if let Some(error) = claim_required {
                Err(error)
            } else if !context.status.switch_allowed {
                Err(AppError::PreflightRefused {
                    reasons: context.status.refusal_reasons,
                })
            } else if let Some(error) = expectation_error {
                Err(error)
            } else {
                Ok(())
            }
        }
        Err(error) => {
            let report = PreflightReport::from_error(name, &error);
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print_preflight_report(&report);
            }
            Err(error)
        }
    }
}

fn build_preflight_claim_reports(
    context: &StatusContext,
    agent: String,
    claim_required: bool,
    stale_after_seconds: Option<u64>,
) -> Result<(PreflightClaimReport, PreflightClaimReport), AppError> {
    let store = ClaimStore::for_repository(&context.repository);
    let claims = store.load()?;
    let claim_report = build_claim_report_from_claims(
        &store,
        &claims,
        &context.status.pair,
        &context.status.current,
        agent.clone(),
        claim_required,
        stale_after_seconds,
    )?;
    let target_claim_report = build_claim_report_from_claims(
        &store,
        &claims,
        &context.status.pair,
        &context.status.other,
        agent,
        false,
        stale_after_seconds,
    )?;

    Ok((claim_report, target_claim_report))
}

fn build_claim_report_for_scope(
    repository: &GitRepository,
    pair: &str,
    branch: &str,
    agent: String,
    claim_required: bool,
    stale_after_seconds: Option<u64>,
) -> Result<PreflightClaimReport, AppError> {
    let store = ClaimStore::for_repository(repository);
    let claims = store.load()?;
    build_claim_report_from_claims(
        &store,
        &claims,
        pair,
        branch,
        agent,
        claim_required,
        stale_after_seconds,
    )
}

fn build_claim_report_from_claims(
    store: &ClaimStore,
    claims: &AgentClaims,
    pair: &str,
    branch: &str,
    agent: String,
    claim_required: bool,
    stale_after_seconds: Option<u64>,
) -> Result<PreflightClaimReport, AppError> {
    let metadata_lock = build_metadata_lock_report(&store.lock_path());
    let conflict = claims.conflict_for_scope(&agent, pair, branch).cloned();
    let owned_claim = claims.get_for_scope(&agent, pair, branch).cloned();
    let now_unix = if stale_after_seconds.is_some() && conflict.is_some() {
        Some(current_unix_timestamp()?)
    } else {
        None
    };
    let conflict_stale = conflict.as_ref().and_then(|conflict| {
        stale_after_seconds.map(|stale_after_seconds| {
            claim_is_stale(
                conflict,
                now_unix.expect("stale claim conflict reporting has a timestamp"),
                stale_after_seconds,
            )
        })
    });

    Ok(PreflightClaimReport {
        requested_agent: agent,
        claim_allowed: conflict.is_none() && metadata_lock.ok,
        claim_required,
        claim_owned: owned_claim.is_some(),
        owned_claim,
        metadata_lock,
        stale_after_seconds,
        conflict_stale,
        conflict,
    })
}

fn build_branch_expectation_report(
    context: &StatusContext,
    expected_branch: Option<String>,
    expected_side: Option<PairSide>,
) -> Option<BranchExpectationReport> {
    build_branch_expectation_report_for_pair(
        &context.pair,
        &context.status.pair,
        &context.status.current,
        expected_branch,
        expected_side,
    )
}

fn build_branch_expectation_report_for_pair(
    pair: &BranchPair,
    pair_name: &str,
    current_branch: &str,
    expected_branch: Option<String>,
    expected_side: Option<PairSide>,
) -> Option<BranchExpectationReport> {
    if expected_branch.is_none() && expected_side.is_none() {
        return None;
    }

    let current_side = pair_side_for_branch(pair, current_branch);
    let mut failures = Vec::new();

    if let Some(expected_branch) = &expected_branch
        && current_branch != expected_branch.as_str()
    {
        failures.push(format!(
            "current branch '{}' did not match expected branch '{}'",
            current_branch, expected_branch
        ));
    }

    if let Some(expected_side) = expected_side
        && current_side != Some(expected_side)
    {
        failures.push(format!(
            "current branch '{}' is not the {} side of pair '{}'",
            current_branch,
            pair_side_name(expected_side),
            pair_name
        ));
    }

    Some(BranchExpectationReport {
        ok: failures.is_empty(),
        expected_branch,
        expected_side: expected_side.map(pair_side_name),
        current_side: current_side.map(pair_side_name),
        failures,
    })
}

fn assert_repository_state(
    json: bool,
    pair_name: Option<String>,
    expected_branch: Option<String>,
    expected_side: Option<PairSide>,
    agent: Option<String>,
    require_claim: bool,
) -> Result<(), AppError> {
    if let Some(agent) = &agent {
        validate_agent_name(agent)?;
    }
    let repository = GitRepository::discover(".")?;
    let current_branch = repository.current_branch()?;
    let effective_pair = pair_name
        .or_else(|| expected_side.map(|_| "default".to_owned()))
        .or_else(|| agent.as_ref().map(|_| "default".to_owned()))
        .or_else(|| {
            if expected_branch.is_none() {
                Some("default".to_owned())
            } else {
                None
            }
        });
    let mut failures = Vec::new();

    if let Some(expected_branch) = &expected_branch
        && current_branch != *expected_branch
    {
        failures.push(format!(
            "current branch '{current_branch}' did not match expected branch '{expected_branch}'"
        ));
    }

    let mut pair_report = None;
    let mut claim_scope_pair = None;
    if let Some(pair_name) = effective_pair {
        let store = MetadataStore::for_repository(&repository);
        let pairs = store.load()?;

        match pairs.get(&pair_name) {
            Some(pair) => {
                let current_side = pair_side_for_branch(pair, &current_branch);

                if current_side.is_none() {
                    failures.push(format!(
                        "current branch '{current_branch}' is not part of pair '{pair_name}'"
                    ));
                }

                if current_side.is_some() {
                    claim_scope_pair = Some(pair.name.clone());
                }

                if let Some(expected_side) = expected_side
                    && current_side != Some(expected_side)
                {
                    failures.push(format!(
                        "current branch '{current_branch}' is not the {} side of pair '{pair_name}'",
                        pair_side_name(expected_side)
                    ));
                }

                pair_report = Some(AssertPairReport {
                    name: pair.name.clone(),
                    left: Some(pair.left.clone()),
                    right: Some(pair.right.clone()),
                    configured: true,
                    current_side: current_side.map(pair_side_name),
                    expected_side: expected_side.map(pair_side_name),
                });
            }
            None => {
                failures.push(format!("pair '{pair_name}' was not found"));
                pair_report = Some(AssertPairReport {
                    name: pair_name,
                    left: None,
                    right: None,
                    configured: false,
                    current_side: None,
                    expected_side: expected_side.map(pair_side_name),
                });
            }
        }
    }

    let mut claim_conflict = None;
    let mut claim_blocked = None;
    let mut claim_required = None;
    let claim = if let (Some(agent), Some(pair)) = (agent, claim_scope_pair) {
        let claim_report = build_claim_report_for_scope(
            &repository,
            &pair,
            &current_branch,
            agent,
            require_claim,
            None,
        )?;
        let conflict = claim_report.conflict.clone();
        if conflict.is_none() && !claim_report.metadata_lock.ok {
            claim_blocked = Some(AppError::ClaimBlocked {
                agent: claim_report.requested_agent.clone(),
                pair: pair.clone(),
                branch: current_branch.clone(),
                reason: metadata_lock_block_reason(&claim_report.metadata_lock),
            });
        }
        if require_claim && conflict.is_none() && !claim_report.claim_owned {
            claim_required = Some(AppError::ClaimRequired {
                agent: claim_report.requested_agent.clone(),
                pair: pair.clone(),
                branch: current_branch.clone(),
            });
        }
        claim_conflict = conflict;
        Some(claim_report)
    } else {
        None
    };

    let report = AssertReport {
        ok: failures.is_empty()
            && claim
                .as_ref()
                .is_none_or(|claim| claim.claim_allowed && (!require_claim || claim.claim_owned)),
        repository_root: repository.root().display().to_string(),
        current_branch,
        expected_branch,
        pair: pair_report,
        claim,
        failures,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_assert_report(&report);
    }

    if let Some(conflict) = claim_conflict {
        Err(AppError::ClaimConflict {
            agent: conflict.agent,
            pair: conflict.pair,
            branch: conflict.branch,
        })
    } else if let Some(error) = claim_blocked {
        Err(error)
    } else if let Some(error) = claim_required {
        Err(error)
    } else if !report.ok {
        Err(AppError::AssertFailed {
            failures: report.failures,
        })
    } else {
        Ok(())
    }
}

struct ClaimScopeOptions {
    json: bool,
    agent: String,
    pair_name: String,
    expected_branch: Option<String>,
    expected_side: Option<PairSide>,
    note: Option<String>,
    clear_note: bool,
    stale_after: Option<String>,
}

fn claim_current_scope(options: ClaimScopeOptions) -> Result<(), AppError> {
    let ClaimScopeOptions {
        json,
        agent,
        pair_name,
        expected_branch,
        expected_side,
        note,
        clear_note,
        stale_after,
    } = options;
    validate_agent_name(&agent)?;
    if let Some(note) = &note {
        validate_claim_note(note)?;
    }
    let stale_after_seconds = stale_after
        .as_deref()
        .map(parse_duration_seconds)
        .transpose()?;
    let repository = GitRepository::discover(".")?;
    let store = ClaimStore::for_repository(&repository);
    let _lock = store.lock()?;
    let context = load_status_context(&pair_name)?;
    let expectation = build_branch_expectation_report(&context, expected_branch, expected_side);
    let expectation_error = expectation.as_ref().and_then(|expectation| {
        (!expectation.ok).then(|| AppError::AssertFailed {
            failures: expectation.failures.clone(),
        })
    });

    if !context.status.switch_allowed {
        let report = ClaimOperationReport {
            ok: false,
            status: "refused",
            repository_root: context.repository.root().display().to_string(),
            claims_path: store.path().display().to_string(),
            agent,
            pair: pair_name,
            branch: context.status.current,
            claim: None,
            conflict: None,
            expectation,
            refusal_reasons: context.status.refusal_reasons.clone(),
            stale_after_seconds,
            conflict_stale: None,
        };
        print_claim_operation_report(&report, json)?;
        return Err(AppError::PreflightRefused {
            reasons: context.status.refusal_reasons,
        });
    }

    if let Some(error) = expectation_error {
        let report = ClaimOperationReport {
            ok: false,
            status: "refused",
            repository_root: context.repository.root().display().to_string(),
            claims_path: store.path().display().to_string(),
            agent,
            pair: pair_name,
            branch: context.status.current,
            claim: None,
            conflict: None,
            expectation,
            refusal_reasons: Vec::new(),
            stale_after_seconds,
            conflict_stale: None,
        };
        print_claim_operation_report(&report, json)?;
        return Err(error);
    }

    let mut claims = store.load()?;
    if let Some(conflict) =
        claims.conflict_for_scope(&agent, &context.status.pair, &context.status.current)
    {
        let conflict_agent = conflict.agent.clone();
        let now_unix = if stale_after_seconds.is_some() {
            Some(current_unix_timestamp()?)
        } else {
            None
        };
        let conflict_stale = stale_after_seconds.map(|stale_after_seconds| {
            claim_is_stale(
                conflict,
                now_unix.expect("stale claim conflict reporting has a timestamp"),
                stale_after_seconds,
            )
        });
        let report = ClaimOperationReport {
            ok: false,
            status: "conflict",
            repository_root: context.repository.root().display().to_string(),
            claims_path: store.path().display().to_string(),
            agent,
            pair: context.status.pair,
            branch: context.status.current,
            claim: None,
            conflict: Some(conflict.clone()),
            expectation,
            refusal_reasons: Vec::new(),
            stale_after_seconds,
            conflict_stale,
        };
        print_claim_operation_report(&report, json)?;
        return Err(AppError::ClaimConflict {
            agent: conflict_agent,
            pair: report.pair,
            branch: report.branch,
        });
    }

    let existing_note = claims
        .get_for_scope(&agent, &context.status.pair, &context.status.current)
        .and_then(|claim| claim.note.clone());
    let next_note = if clear_note {
        None
    } else {
        note.or(existing_note)
    };
    let claim = AgentClaim::new_with_note(
        agent,
        context.status.pair,
        context.status.current,
        current_unix_timestamp()?,
        next_note,
    )?;
    claims.upsert(claim.clone());
    store.save(&claims)?;

    let report = ClaimOperationReport {
        ok: true,
        status: "claimed",
        repository_root: context.repository.root().display().to_string(),
        claims_path: store.path().display().to_string(),
        agent: claim.agent.clone(),
        pair: claim.pair.clone(),
        branch: claim.branch.clone(),
        claim: Some(claim),
        conflict: None,
        expectation,
        refusal_reasons: Vec::new(),
        stale_after_seconds,
        conflict_stale: None,
    };
    print_claim_operation_report(&report, json)?;

    Ok(())
}

fn heartbeat_claim(options: ClaimScopeOptions) -> Result<(), AppError> {
    let ClaimScopeOptions {
        json,
        agent,
        pair_name,
        expected_branch,
        expected_side,
        note,
        clear_note,
        stale_after,
    } = options;
    validate_agent_name(&agent)?;
    if let Some(note) = &note {
        validate_claim_note(note)?;
    }
    let stale_after_seconds = stale_after
        .as_deref()
        .map(parse_duration_seconds)
        .transpose()?;
    let repository = GitRepository::discover(".")?;
    let store = ClaimStore::for_repository(&repository);
    let _lock = store.lock()?;
    let context = load_status_context(&pair_name)?;
    let expectation = build_branch_expectation_report(&context, expected_branch, expected_side);
    let expectation_error = expectation.as_ref().and_then(|expectation| {
        (!expectation.ok).then(|| AppError::AssertFailed {
            failures: expectation.failures.clone(),
        })
    });

    if let Some(error) = expectation_error {
        let report = ClaimOperationReport {
            ok: false,
            status: "refused",
            repository_root: context.repository.root().display().to_string(),
            claims_path: store.path().display().to_string(),
            agent,
            pair: pair_name,
            branch: context.status.current,
            claim: None,
            conflict: None,
            expectation,
            refusal_reasons: Vec::new(),
            stale_after_seconds,
            conflict_stale: None,
        };
        print_claim_operation_report(&report, json)?;
        return Err(error);
    }

    let mut claims = store.load()?;

    if let Some(conflict) =
        claims.conflict_for_scope(&agent, &context.status.pair, &context.status.current)
    {
        let conflict_agent = conflict.agent.clone();
        let now_unix = if stale_after_seconds.is_some() {
            Some(current_unix_timestamp()?)
        } else {
            None
        };
        let conflict_stale = stale_after_seconds.map(|stale_after_seconds| {
            claim_is_stale(
                conflict,
                now_unix.expect("stale claim conflict reporting has a timestamp"),
                stale_after_seconds,
            )
        });
        let report = ClaimOperationReport {
            ok: false,
            status: "conflict",
            repository_root: context.repository.root().display().to_string(),
            claims_path: store.path().display().to_string(),
            agent,
            pair: context.status.pair,
            branch: context.status.current,
            claim: None,
            conflict: Some(conflict.clone()),
            expectation,
            refusal_reasons: Vec::new(),
            stale_after_seconds,
            conflict_stale,
        };
        print_claim_operation_report(&report, json)?;
        return Err(AppError::ClaimConflict {
            agent: conflict_agent,
            pair: report.pair,
            branch: report.branch,
        });
    }

    let previous_claim = claims
        .get_for_scope(&agent, &context.status.pair, &context.status.current)
        .cloned()
        .ok_or_else(|| AppError::ClaimNotFound {
            agent: agent.clone(),
            pair: context.status.pair.clone(),
            branch: context.status.current.clone(),
        })?;

    let claim = AgentClaim::new_with_note(
        agent,
        context.status.pair,
        context.status.current,
        current_unix_timestamp()?,
        if clear_note {
            None
        } else {
            note.or(previous_claim.note)
        },
    )?;
    claims.upsert(claim.clone());
    store.save(&claims)?;

    let report = ClaimOperationReport {
        ok: true,
        status: "refreshed",
        repository_root: context.repository.root().display().to_string(),
        claims_path: store.path().display().to_string(),
        agent: claim.agent.clone(),
        pair: claim.pair.clone(),
        branch: claim.branch.clone(),
        claim: Some(claim),
        conflict: None,
        expectation,
        refusal_reasons: Vec::new(),
        stale_after_seconds,
        conflict_stale: None,
    };
    print_claim_operation_report(&report, json)?;

    Ok(())
}

struct ListClaimsOptions {
    json: bool,
    agent: Option<String>,
    conflicts_for: Option<String>,
    pair: Option<String>,
    branch: Option<String>,
    current: bool,
    target: bool,
    side: Option<PairSide>,
    stale_after: Option<String>,
}

fn list_claims(options: ListClaimsOptions) -> Result<(), AppError> {
    let ListClaimsOptions {
        json,
        agent,
        conflicts_for,
        mut pair,
        branch,
        current,
        target,
        side,
        stale_after,
    } = options;

    if let Some(agent) = &agent {
        validate_agent_name(agent)?;
    }
    if let Some(conflicts_for) = &conflicts_for {
        validate_agent_name(conflicts_for)?;
    }
    if let Some(pair) = &pair {
        validate_pair_name(pair)?;
    }
    let stale_after_seconds = stale_after
        .as_deref()
        .map(parse_duration_seconds)
        .transpose()?;
    let now_unix = if stale_after_seconds.is_some() {
        Some(current_unix_timestamp()?)
    } else {
        None
    };

    let repository = GitRepository::discover(".")?;
    let branch = if current {
        Some(repository.current_branch()?)
    } else if target {
        let context = load_status_context(pair.as_deref().unwrap_or("default"))?;
        pair = Some(context.status.pair);
        Some(context.status.other)
    } else if let Some(side) = side {
        Some(resolve_pair_side_branch(&repository, &mut pair, side)?)
    } else {
        branch
    };
    if let Some(branch) = &branch {
        ensure_branch_name_is_valid(&repository, branch)?;
    }

    let store = ClaimStore::for_repository(&repository);
    let claims = store.load()?;
    let filtered_claims = claims
        .claims()
        .iter()
        .filter(|claim| {
            claim_matches_filters(claim, agent.as_deref(), pair.as_deref(), branch.as_deref())
                && conflicts_for
                    .as_deref()
                    .is_none_or(|conflicts_for| claim.agent != conflicts_for)
                && stale_after_seconds.is_none_or(|stale_after_seconds| {
                    claim_is_stale(
                        claim,
                        now_unix.expect("stale claim filtering has a timestamp"),
                        stale_after_seconds,
                    )
                })
        })
        .cloned()
        .collect();
    let report = ClaimsReport {
        repository_root: repository.root().display().to_string(),
        claims_path: store.path().display().to_string(),
        filters: ClaimsFilterReport {
            agent,
            conflicts_for,
            pair,
            branch,
            current,
            target,
            side: side.map(pair_side_name),
            stale_after_seconds,
        },
        claims: filtered_claims,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    print_claims_report(&report);
    Ok(())
}

struct PruneClaimsOptions {
    json: bool,
    agent: Option<String>,
    pair: Option<String>,
    branch: Option<String>,
    current: bool,
    target: bool,
    side: Option<PairSide>,
    stale_after: Option<String>,
    orphaned: bool,
    apply: bool,
}

fn prune_claims(options: PruneClaimsOptions) -> Result<(), AppError> {
    let PruneClaimsOptions {
        json,
        agent,
        pair,
        branch,
        current,
        target,
        side,
        stale_after,
        orphaned,
        apply,
    } = options;

    if let Some(agent) = &agent {
        validate_agent_name(agent)?;
    }
    if let Some(pair) = &pair {
        validate_pair_name(pair)?;
    }
    let stale_after_seconds = stale_after
        .as_deref()
        .map(parse_duration_seconds)
        .transpose()?;
    let now_unix = if stale_after_seconds.is_some() {
        Some(current_unix_timestamp()?)
    } else {
        None
    };

    let repository = GitRepository::discover(".")?;
    let mut pair = pair;
    let branch = if current {
        Some(repository.current_branch()?)
    } else if target {
        let context = load_status_context(pair.as_deref().unwrap_or("default"))?;
        pair = Some(context.status.pair);
        Some(context.status.other)
    } else if let Some(side) = side {
        Some(resolve_pair_side_branch(&repository, &mut pair, side)?)
    } else {
        branch
    };
    if let Some(branch) = &branch {
        ensure_branch_name_is_valid(&repository, branch)?;
    }

    let store = ClaimStore::for_repository(&repository);
    let _lock = if apply { Some(store.lock()?) } else { None };
    let mut claims = store.load()?;
    let pairs = if orphaned {
        Some(MetadataStore::for_repository(&repository).load()?)
    } else {
        None
    };
    let pruned_claim_issues = if orphaned {
        diagnose_claim_issues(&repository, pairs.as_ref(), claims.claims())
            .into_iter()
            .filter(|issue| {
                claim_matches_filters(
                    &issue.claim,
                    agent.as_deref(),
                    pair.as_deref(),
                    branch.as_deref(),
                )
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let pruned_claims = claims
        .claims()
        .iter()
        .filter(|claim| {
            if !claim_matches_filters(claim, agent.as_deref(), pair.as_deref(), branch.as_deref()) {
                return false;
            }

            let stale_matches = stale_after_seconds.is_some_and(|stale_after_seconds| {
                claim_is_stale(
                    claim,
                    now_unix.expect("stale claim pruning has a timestamp"),
                    stale_after_seconds,
                )
            });
            let orphaned_matches = pruned_claim_issues
                .iter()
                .any(|issue| same_claim_scope(&issue.claim, claim));

            stale_matches || orphaned_matches
        })
        .cloned()
        .collect::<Vec<_>>();

    if apply {
        for claim in &pruned_claims {
            claims.remove(&claim.agent, &claim.pair, &claim.branch);
        }
        if !pruned_claims.is_empty() {
            store.save(&claims)?;
        }
    }

    let report = PruneClaimsReport {
        applied: apply,
        repository_root: repository.root().display().to_string(),
        claims_path: store.path().display().to_string(),
        filters: ClaimsFilterReport {
            agent,
            conflicts_for: None,
            pair,
            branch,
            current,
            target,
            side: side.map(pair_side_name),
            stale_after_seconds,
        },
        orphaned,
        pruned_claims,
        pruned_claim_issues,
        remaining_claim_count: claims.claims().len(),
    };

    print_prune_claims_report(&report, json)
}

fn unclaim_current_scope(
    json: bool,
    agent: String,
    pair_name: String,
    branch: Option<String>,
    side: Option<PairSide>,
) -> Result<(), AppError> {
    validate_agent_name(&agent)?;
    let repository = GitRepository::discover(".")?;
    let branch = if let Some(branch) = branch {
        ensure_branch_name_is_valid(&repository, &branch)?;
        branch
    } else if let Some(side) = side {
        let pairs = MetadataStore::for_repository(&repository).load()?;
        let pair = pairs
            .get(&pair_name)
            .ok_or_else(|| AppError::PairNotFound {
                name: pair_name.clone(),
            })?;
        match side {
            PairSide::Left => pair.left.clone(),
            PairSide::Right => pair.right.clone(),
        }
    } else {
        repository.current_branch()?
    };
    let store = ClaimStore::for_repository(&repository);
    let _lock = store.lock()?;
    let mut claims = store.load()?;

    let removed =
        claims
            .remove(&agent, &pair_name, &branch)
            .ok_or_else(|| AppError::ClaimNotFound {
                agent: agent.clone(),
                pair: pair_name.clone(),
                branch: branch.clone(),
            })?;
    store.save(&claims)?;

    let report = ClaimOperationReport {
        ok: true,
        status: "released",
        repository_root: repository.root().display().to_string(),
        claims_path: store.path().display().to_string(),
        agent,
        pair: pair_name,
        branch,
        claim: Some(removed),
        conflict: None,
        expectation: None,
        refusal_reasons: Vec::new(),
        stale_after_seconds: None,
        conflict_stale: None,
    };
    print_claim_operation_report(&report, json)?;

    Ok(())
}

fn show_handoff(
    json: bool,
    pair_name: String,
    expected_branch: Option<String>,
    expected_side: Option<PairSide>,
    agent: Option<String>,
    require_claim: bool,
    stale_after: Option<String>,
) -> Result<(), AppError> {
    if let Some(agent) = &agent {
        validate_agent_name(agent)?;
    }
    let stale_after_seconds = stale_after
        .as_deref()
        .map(parse_duration_seconds)
        .transpose()?;

    let mut report = HandoffReport {
        ok: false,
        generated_at_unix: current_unix_timestamp()?,
        requested_pair: pair_name.clone(),
        requested_agent: agent,
        repository_root: None,
        current_branch: None,
        worktree: None,
        git_state: None,
        expectation: None,
        pair: None,
        claims: Vec::new(),
        claim: None,
        target_claim: None,
        errors: Vec::new(),
    };

    let repository = match GitRepository::discover(".") {
        Ok(repository) => repository,
        Err(error) => return finish_handoff_with_error(report, AppError::from(error), json),
    };
    report.repository_root = Some(repository.root().display().to_string());

    let current_branch = match repository.current_branch() {
        Ok(current_branch) => current_branch,
        Err(error) => return finish_handoff_with_error(report, AppError::from(error), json),
    };
    report.current_branch = Some(current_branch.clone());

    let is_dirty = match repository.is_dirty() {
        Ok(is_dirty) => is_dirty,
        Err(error) => return finish_handoff_with_error(report, AppError::from(error), json),
    };
    report.worktree = Some(WorktreeStatus::from_dirty(is_dirty));

    let is_merge_in_progress = repository.is_merge_in_progress();
    let is_rebase_in_progress = repository.is_rebase_in_progress();
    report.git_state = Some(GitState::from_repository_state(
        is_merge_in_progress,
        is_rebase_in_progress,
    ));

    let claim_store = ClaimStore::for_repository(&repository);
    let claims = match claim_store.load() {
        Ok(claims) => claims,
        Err(error) => return finish_handoff_with_error(report, AppError::from(error), json),
    };
    report.claims = claims.claims().to_vec();

    let store = MetadataStore::for_repository(&repository);
    let pairs = match store.load() {
        Ok(pairs) => pairs,
        Err(error) => return finish_handoff_with_error(report, AppError::from(error), json),
    };
    let pair = match pairs.get(&pair_name) {
        Some(pair) => pair,
        None => {
            return finish_handoff_with_error(
                report,
                AppError::PairNotFound { name: pair_name },
                json,
            );
        }
    };

    let pair_report = match build_status_report(
        &repository,
        pair,
        &current_branch,
        is_dirty,
        is_merge_in_progress,
        is_rebase_in_progress,
    ) {
        Ok(pair_report) => pair_report,
        Err(error) => return finish_handoff_with_error(report, error, json),
    };

    let expectation = build_branch_expectation_report_for_pair(
        pair,
        &pair_report.pair,
        &pair_report.current,
        expected_branch,
        expected_side,
    );
    let expectation_error = expectation.as_ref().and_then(|expectation| {
        (!expectation.ok).then(|| AppError::AssertFailed {
            failures: expectation.failures.clone(),
        })
    });
    let claim_pair = pair_report.pair.clone();
    let claim_branch = pair_report.current.clone();
    let target_claim_branch = pair_report.other.clone();
    report.expectation = expectation;
    report.pair = Some(pair_report);

    if let Some(error) = expectation_error {
        return finish_handoff_with_error(report, error, json);
    }

    if let Some(agent) = report.requested_agent.clone() {
        let claim_report = build_claim_report_from_claims(
            &claim_store,
            &claims,
            &claim_pair,
            &claim_branch,
            agent.clone(),
            require_claim,
            stale_after_seconds,
        )?;
        let required_claim_error = if require_claim {
            if let Some(conflict) = &claim_report.conflict {
                Some(AppError::ClaimConflict {
                    agent: conflict.agent.clone(),
                    pair: conflict.pair.clone(),
                    branch: conflict.branch.clone(),
                })
            } else if !claim_report.claim_owned {
                Some(AppError::ClaimRequired {
                    agent: agent.clone(),
                    pair: claim_pair.clone(),
                    branch: claim_branch.clone(),
                })
            } else {
                None
            }
        } else {
            None
        };

        report.claim = Some(claim_report);

        if let Some(target_branch) = target_claim_branch {
            report.target_claim = Some(build_claim_report_from_claims(
                &claim_store,
                &claims,
                &claim_pair,
                &target_branch,
                agent,
                false,
                stale_after_seconds,
            )?);
        }

        if let Some(error) = required_claim_error {
            return finish_handoff_with_error(report, error, json);
        }
    }

    report.ok = true;
    print_handoff_report(&report, json)
}

fn finish_handoff_with_error(
    mut report: HandoffReport,
    error: AppError,
    json: bool,
) -> Result<(), AppError> {
    report.errors.push(HandoffErrorReport::from_error(&error));
    print_handoff_report(&report, json)?;
    Err(error)
}

fn run_doctor(json: bool, stale_after: Option<String>) -> Result<(), AppError> {
    let stale_after_seconds = stale_after
        .as_deref()
        .map(parse_duration_seconds)
        .transpose()?;
    let report = build_doctor_report(stale_after_seconds);

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_doctor_report(&report);
    }

    if report.healthy {
        Ok(())
    } else {
        Err(AppError::DoctorFailed)
    }
}

fn build_doctor_report(stale_after_seconds: Option<u64>) -> DoctorReport {
    let mut report = DoctorReport::default();

    match GitRepository::version() {
        Ok(version) => {
            report.git.ok = true;
            report.git.version = Some(version);
        }
        Err(error) => {
            report.healthy = false;
            report.git.error = Some(error.to_string());
            return report;
        }
    }

    let repository = match GitRepository::discover(".") {
        Ok(repository) => {
            report.repository.ok = true;
            report.repository.root = Some(repository.root().display().to_string());
            report.repository.git_dir = Some(repository.git_dir().display().to_string());
            repository
        }
        Err(error) => {
            report.healthy = false;
            report.repository.error = Some(error.to_string());
            return report;
        }
    };

    match repository.current_branch() {
        Ok(branch) => {
            report.current_branch = Some(DoctorCurrentBranchReport {
                ok: true,
                branch: Some(branch),
                error: None,
            });
        }
        Err(error) => {
            report.healthy = false;
            report.current_branch = Some(DoctorCurrentBranchReport {
                ok: false,
                branch: None,
                error: Some(error.to_string()),
            });
        }
    }

    match repository.is_dirty() {
        Ok(is_dirty) => {
            report.worktree = Some(DoctorWorktreeReport {
                ok: true,
                state: Some(if is_dirty { "dirty" } else { "clean" }.to_owned()),
                error: None,
            });
        }
        Err(error) => {
            report.healthy = false;
            report.worktree = Some(DoctorWorktreeReport {
                ok: false,
                state: None,
                error: Some(error.to_string()),
            });
        }
    }

    report.git_state = Some(
        format_git_state(
            repository.is_merge_in_progress(),
            repository.is_rebase_in_progress(),
        )
        .to_owned(),
    );

    let store = MetadataStore::for_repository(&repository);
    let metadata_lock_report = build_metadata_lock_report(&store.lock_path());
    if !metadata_lock_report.ok {
        report.healthy = false;
    }
    report.metadata_lock = Some(metadata_lock_report);

    let mut loaded_pairs = None;
    match store.load() {
        Ok(pairs) => {
            let mut pair_reports = Vec::new();

            for pair in pairs.pairs() {
                match diagnose_pair_branches(&repository, pair) {
                    Ok(summary) => {
                        let ok = summary == "ok";
                        if !ok {
                            report.healthy = false;
                        }
                        pair_reports.push(DoctorPairReport {
                            name: pair.name.clone(),
                            left: pair.left.clone(),
                            right: pair.right.clone(),
                            ok,
                            summary,
                        });
                    }
                    Err(error) => {
                        report.healthy = false;
                        pair_reports.push(DoctorPairReport {
                            name: pair.name.clone(),
                            left: pair.left.clone(),
                            right: pair.right.clone(),
                            ok: false,
                            summary: format!("error: {error}"),
                        });
                    }
                }
            }

            loaded_pairs = Some(pairs.clone());
            report.metadata = Some(DoctorMetadataReport {
                ok: true,
                path: Some(store.path().display().to_string()),
                pair_count: Some(pairs.pairs().len()),
                error: None,
                pairs: pair_reports,
            });
        }
        Err(error) => {
            report.healthy = false;
            report.metadata = Some(DoctorMetadataReport {
                ok: false,
                path: Some(store.path().display().to_string()),
                pair_count: None,
                error: Some(error.to_string()),
                pairs: Vec::new(),
            });
        }
    }

    let claim_store = ClaimStore::for_repository(&repository);
    match claim_store.load() {
        Ok(claims) => {
            let now_unix = current_unix_timestamp().ok();
            let claim_issues =
                diagnose_claim_issues(&repository, loaded_pairs.as_ref(), claims.claims());
            let stale_claims = match (stale_after_seconds, now_unix) {
                (Some(stale_after_seconds), Some(now_unix)) => claims
                    .claims()
                    .iter()
                    .filter(|claim| claim_is_stale(claim, now_unix, stale_after_seconds))
                    .cloned()
                    .collect::<Vec<_>>(),
                _ => Vec::new(),
            };

            if !claim_issues.is_empty()
                || !stale_claims.is_empty()
                || (stale_after_seconds.is_some() && now_unix.is_none())
            {
                report.healthy = false;
            }

            report.claims = Some(DoctorClaimsReport {
                ok: claim_issues.is_empty()
                    && stale_after_seconds.is_none_or(|_| now_unix.is_some()),
                path: Some(claim_store.path().display().to_string()),
                claim_count: Some(claims.claims().len()),
                claim_issue_count: Some(claim_issues.len()),
                claim_issues,
                stale_after_seconds,
                stale_claim_count: Some(stale_claims.len()),
                stale_claims,
                error: if stale_after_seconds.is_some() && now_unix.is_none() {
                    Some("system clock is before the Unix epoch".to_owned())
                } else {
                    None
                },
            });
        }
        Err(error) => {
            report.healthy = false;
            report.claims = Some(DoctorClaimsReport {
                ok: false,
                path: Some(claim_store.path().display().to_string()),
                claim_count: None,
                claim_issue_count: None,
                claim_issues: Vec::new(),
                stale_after_seconds,
                stale_claim_count: None,
                stale_claims: Vec::new(),
                error: Some(error.to_string()),
            });
        }
    }

    report
}

fn print_doctor_report(report: &DoctorReport) {
    match (&report.git.version, &report.git.error) {
        (Some(version), _) => println!("Git: ok ({version})"),
        (_, Some(error)) => println!("Git: error ({error})"),
        _ => println!("Git: error (unknown)"),
    }

    if !report.git.ok {
        return;
    }

    match (&report.repository.root, &report.repository.error) {
        (Some(root), _) => println!("Repository: ok ({root})"),
        (_, Some(error)) => {
            println!("Repository: error ({error})");
            return;
        }
        _ => {
            println!("Repository: error (unknown)");
            return;
        }
    }

    if let Some(git_dir) = &report.repository.git_dir {
        println!("Git directory: {git_dir}");
    }

    if let Some(current_branch) = &report.current_branch {
        if let Some(branch) = &current_branch.branch {
            println!("Current branch: {branch}");
        } else if let Some(error) = &current_branch.error {
            println!("Current branch: error ({error})");
        }
    }

    if let Some(worktree) = &report.worktree {
        if let Some(state) = &worktree.state {
            println!("Worktree: {state}");
        } else if let Some(error) = &worktree.error {
            println!("Worktree: error ({error})");
        }
    }

    if let Some(git_state) = &report.git_state {
        println!("Git state: {git_state}");
    }

    if let Some(metadata_lock) = &report.metadata_lock {
        if let Some(error) = &metadata_lock.error {
            println!("Metadata lock: error ({error})");
        } else if metadata_lock.locked.unwrap_or(false) {
            println!(
                "Metadata lock: locked ({})",
                metadata_lock.path.as_deref().unwrap_or("unknown")
            );
        } else {
            println!(
                "Metadata lock: clear ({})",
                metadata_lock.path.as_deref().unwrap_or("unknown")
            );
        }
    }

    if let Some(metadata) = &report.metadata {
        if metadata.ok {
            println!(
                "Metadata: ok ({} pair(s), {})",
                metadata.pair_count.unwrap_or_default(),
                metadata.path.as_deref().unwrap_or("unknown")
            );
            if metadata.pairs.is_empty() {
                println!("Pairs: none configured");
            } else {
                println!("Pairs:");
                for pair in &metadata.pairs {
                    println!(
                        "- {}: {} <-> {} [{}]",
                        pair.name, pair.left, pair.right, pair.summary
                    );
                }
            }
        } else if let Some(error) = &metadata.error {
            println!("Metadata: error ({error})");
        }
    }

    if let Some(claims) = &report.claims {
        if let Some(error) = &claims.error {
            println!("Claims: error ({error})");
        } else {
            let status = if claims.ok { "ok" } else { "problems" };
            println!(
                "Claims: {} ({} claim(s), {})",
                status,
                claims.claim_count.unwrap_or_default(),
                claims.path.as_deref().unwrap_or("unknown")
            );
            if !claims.claim_issues.is_empty() {
                println!(
                    "Claim issues: {}",
                    claims.claim_issue_count.unwrap_or_default()
                );
                for issue in &claims.claim_issues {
                    println!(
                        "- {}: {} on {} [{}]",
                        issue.claim.agent, issue.claim.pair, issue.claim.branch, issue.message
                    );
                }
            }
            if let Some(stale_after_seconds) = claims.stale_after_seconds {
                let stale_claim_count = claims.stale_claim_count.unwrap_or_default();
                println!(
                    "Stale claims: {} older than {}s",
                    stale_claim_count, stale_after_seconds
                );
                if !claims.stale_claims.is_empty() {
                    for claim in &claims.stale_claims {
                        println!("{}", format_claim_line(claim));
                    }
                }
            }
        }
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

    Ok(StatusContext {
        repository,
        pair: pair.clone(),
        status,
    })
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

fn build_metadata_lock_report(lock_path: &Path) -> MetadataLockReport {
    match lock_path.try_exists() {
        Ok(locked) => MetadataLockReport {
            ok: !locked,
            path: Some(lock_path.display().to_string()),
            locked: Some(locked),
            error: None,
        },
        Err(error) => MetadataLockReport {
            ok: false,
            path: Some(lock_path.display().to_string()),
            locked: None,
            error: Some(error.to_string()),
        },
    }
}

fn metadata_lock_block_reason(metadata_lock: &MetadataLockReport) -> String {
    if let Some(error) = &metadata_lock.error {
        return format!("metadata lock state could not be checked: {error}");
    }

    if metadata_lock.locked.unwrap_or(false) {
        return format!(
            "metadata is locked by another Zaphod process ({})",
            metadata_lock.path.as_deref().unwrap_or("unknown")
        );
    }

    "metadata lock state prevents claiming".to_owned()
}

fn diagnose_claim_issues(
    repository: &GitRepository,
    pairs: Option<&BranchPairs>,
    claims: &[AgentClaim],
) -> Vec<DoctorClaimIssueReport> {
    let Some(pairs) = pairs else {
        return Vec::new();
    };

    claims
        .iter()
        .filter_map(|claim| {
            let pair = match pairs.get(&claim.pair) {
                Some(pair) => pair,
                None => {
                    return Some(DoctorClaimIssueReport {
                        claim: claim.clone(),
                        reason: "missing_pair",
                        message: format!("pair '{}' is not configured", claim.pair),
                    });
                }
            };

            if pair_side_for_branch(pair, &claim.branch).is_none() {
                return Some(DoctorClaimIssueReport {
                    claim: claim.clone(),
                    reason: "branch_not_in_pair",
                    message: format!(
                        "branch '{}' is not part of pair '{}'",
                        claim.branch, claim.pair
                    ),
                });
            }

            match repository.branch_exists(&claim.branch) {
                Ok(true) => None,
                Ok(false) => Some(DoctorClaimIssueReport {
                    claim: claim.clone(),
                    reason: "missing_branch",
                    message: format!("branch '{}' was not found", claim.branch),
                }),
                Err(error) => Some(DoctorClaimIssueReport {
                    claim: claim.clone(),
                    reason: "branch_lookup_error",
                    message: error.to_string(),
                }),
            }
        })
        .collect()
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

fn print_preflight_report(report: &PreflightReport) {
    println!(
        "Preflight: {}",
        if report.ready { "ready" } else { "not ready" }
    );
    println!("Pair: {}", report.pair);

    if let Some(repository_root) = &report.repository_root {
        println!("Repository: {repository_root}");
    }
    if let Some(current) = &report.current {
        println!("Current: {current}");
    }
    if let Some(other) = &report.other {
        println!("Other: {other}");
    }
    if let Some(worktree) = report.worktree {
        println!("Worktree: {worktree}");
    }
    if let Some(git_state) = report.git_state {
        println!("Git state: {git_state}");
    }

    if report.switch_allowed {
        println!("Switch: allowed");
    } else if !report.refusal_reasons.is_empty() {
        let reasons = report
            .refusal_reasons
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        println!("Switch: refused ({reasons})");
    }

    if let Some(expectation) = &report.expectation {
        print_branch_expectation_report(expectation);
    }

    if let Some(claim) = &report.claim {
        print_claim_readiness("Claim", claim);

        if let Some(error) = &claim.metadata_lock.error {
            println!("Claim metadata lock: error ({error})");
        } else if claim.metadata_lock.locked.unwrap_or(false) {
            println!(
                "Claim metadata lock: locked ({})",
                claim.metadata_lock.path.as_deref().unwrap_or("unknown")
            );
        } else {
            println!(
                "Claim metadata lock: clear ({})",
                claim.metadata_lock.path.as_deref().unwrap_or("unknown")
            );
        }
    }
    if let Some(target_claim) = &report.target_claim {
        print_claim_readiness("Target claim", target_claim);
    }

    if let Some(error) = &report.error {
        println!("Error: {}", error.message);
    }
}

fn print_switch_target_claim(report: &SwitchReport) {
    let Some(claim) = &report.target_claim else {
        return;
    };

    if claim.claim_owned {
        println!(
            "Target claim: owned by {} on {}",
            claim.requested_agent, report.target
        );
    } else if claim.claim_required {
        println!(
            "Target claim: missing for {} on {}",
            claim.requested_agent, report.target
        );
    } else if claim.claim_allowed {
        println!(
            "Target claim: allowed for {} on {}",
            claim.requested_agent, report.target
        );
    } else if let Some(conflict) = &claim.conflict {
        println!(
            "Target claim: refused ({} is claimed by {})",
            report.target, conflict.agent
        );
    } else {
        println!("Target claim: refused");
    }
}

fn print_assert_report(report: &AssertReport) {
    println!("Assert: {}", if report.ok { "passed" } else { "failed" });
    println!("Repository: {}", report.repository_root);
    println!("Current: {}", report.current_branch);

    if let Some(expected_branch) = &report.expected_branch {
        println!("Expected branch: {expected_branch}");
    }

    if let Some(pair) = &report.pair {
        if pair.configured {
            let left = pair.left.as_deref().unwrap_or("unknown");
            let right = pair.right.as_deref().unwrap_or("unknown");
            println!("Pair: {} ({} <-> {})", pair.name, left, right);
            if let Some(current_side) = pair.current_side {
                println!("Current side: {current_side}");
            }
            if let Some(expected_side) = pair.expected_side {
                println!("Expected side: {expected_side}");
            }
        } else {
            println!("Pair: {} (missing)", pair.name);
        }
    }

    if let Some(claim) = &report.claim {
        print_claim_readiness("Claim", claim);
    }

    for failure in &report.failures {
        println!("Failure: {failure}");
    }
}

fn print_claim_readiness(label: &str, claim: &PreflightClaimReport) {
    if claim.claim_required && claim.claim_owned {
        println!("{label}: owned by {}", claim.requested_agent);
    } else if claim.claim_required && !claim.claim_owned && claim.conflict.is_none() {
        println!("{label}: missing for {}", claim.requested_agent);
    } else if claim.claim_allowed {
        println!("{label}: allowed for {}", claim.requested_agent);
    } else if let Some(conflict) = &claim.conflict {
        let stale = match (claim.conflict_stale, claim.stale_after_seconds) {
            (Some(true), Some(seconds)) => format!(", stale after {seconds}s"),
            (Some(false), Some(seconds)) => format!(", not stale after {seconds}s"),
            _ => String::new(),
        };
        println!("{label}: refused (claimed by {}{})", conflict.agent, stale);
    } else if let Some(error) = &claim.metadata_lock.error {
        println!("{label}: refused (metadata lock error: {error})");
    } else if claim.metadata_lock.locked.unwrap_or(false) {
        println!(
            "{label}: refused (metadata lock present at {})",
            claim.metadata_lock.path.as_deref().unwrap_or("unknown")
        );
    } else {
        println!("{label}: refused");
    }
}

fn print_branch_expectation_report(expectation: &BranchExpectationReport) {
    println!(
        "Expectation: {}",
        if expectation.ok { "passed" } else { "failed" }
    );
    if let Some(expected_branch) = &expectation.expected_branch {
        println!("Expected branch: {expected_branch}");
    }
    if let Some(expected_side) = expectation.expected_side {
        println!("Expected side: {expected_side}");
    }
    if let Some(current_side) = expectation.current_side {
        println!("Current side: {current_side}");
    }
    if !expectation.failures.is_empty() {
        println!("Expectation failures:");
        for failure in &expectation.failures {
            println!("- {failure}");
        }
    }
}

fn print_claim_operation_report(report: &ClaimOperationReport, json: bool) -> Result<(), AppError> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }

    println!("Claim: {}", report.status);
    println!("Repository: {}", report.repository_root);
    println!("Claims metadata: {}", report.claims_path);
    println!("Agent: {}", report.agent);
    println!("Pair: {}", report.pair);
    println!("Branch: {}", report.branch);
    if let Some(claim) = &report.claim
        && let Some(note) = &claim.note
    {
        println!("Note: {note}");
    }

    if let Some(conflict) = &report.conflict {
        println!("Conflict: claimed by {}", conflict.agent);
        if let Some(note) = &conflict.note {
            println!("Conflict note: {note}");
        }
        if let Some(conflict_stale) = report.conflict_stale {
            println!("Conflict stale: {conflict_stale}");
        }
    }

    if let Some(expectation) = &report.expectation {
        print_branch_expectation_report(expectation);
    }

    if !report.refusal_reasons.is_empty() {
        let reasons = report
            .refusal_reasons
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        println!("Refused: {reasons}");
    }

    Ok(())
}

fn print_claims_report(report: &ClaimsReport) {
    println!("Repository: {}", report.repository_root);
    println!("Claims metadata: {}", report.claims_path);

    if report.filters.agent.is_some()
        || report.filters.conflicts_for.is_some()
        || report.filters.pair.is_some()
        || report.filters.branch.is_some()
        || report.filters.current
        || report.filters.target
        || report.filters.side.is_some()
        || report.filters.stale_after_seconds.is_some()
    {
        println!(
            "Filters: agent={}, conflicts_for={}, pair={}, branch={}, current={}, target={}, side={}, stale_after={}",
            report.filters.agent.as_deref().unwrap_or("*"),
            report.filters.conflicts_for.as_deref().unwrap_or("*"),
            report.filters.pair.as_deref().unwrap_or("*"),
            report.filters.branch.as_deref().unwrap_or("*"),
            report.filters.current,
            report.filters.target,
            report.filters.side.unwrap_or("*"),
            report
                .filters
                .stale_after_seconds
                .map(|seconds| format!("{seconds}s"))
                .unwrap_or_else(|| "*".to_owned())
        );
    }

    if report.claims.is_empty() {
        println!("No active agent claims.");
        return;
    }

    println!("Claims:");
    for claim in &report.claims {
        println!("{}", format_claim_line(claim));
    }
}

fn print_prune_claims_report(report: &PruneClaimsReport, json: bool) -> Result<(), AppError> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }

    println!(
        "Prune claims: {}",
        if report.applied { "applied" } else { "dry-run" }
    );
    println!("Repository: {}", report.repository_root);
    println!("Claims metadata: {}", report.claims_path);
    let stale_after = report
        .filters
        .stale_after_seconds
        .map(|seconds| format!("{seconds}s"))
        .unwrap_or_else(|| "*".to_owned());
    println!(
        "Filters: agent={}, pair={}, branch={}, stale_after={}, orphaned={}",
        report.filters.agent.as_deref().unwrap_or("*"),
        report.filters.pair.as_deref().unwrap_or("*"),
        report.filters.branch.as_deref().unwrap_or("*"),
        stale_after,
        report.orphaned
    );

    if report.pruned_claims.is_empty() {
        println!("No claims matched.");
        return Ok(());
    }

    println!("Matched claims:");
    for claim in &report.pruned_claims {
        println!("{}", format_claim_line(claim));
    }
    if !report.pruned_claim_issues.is_empty() {
        println!("Orphaned claim issues:");
        for issue in &report.pruned_claim_issues {
            println!(
                "- {}: {} on {} [{}]",
                issue.claim.agent, issue.claim.pair, issue.claim.branch, issue.message
            );
        }
    }

    if report.applied {
        println!("Removed {} claim(s).", report.pruned_claims.len());
    } else {
        println!("No metadata changed. Rerun with --apply to remove these claims.");
    }
    println!("Remaining claims: {}", report.remaining_claim_count);

    Ok(())
}

fn print_handoff_report(report: &HandoffReport, json: bool) -> Result<(), AppError> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }

    println!(
        "Handoff: {}",
        if report.ok { "ready" } else { "incomplete" }
    );
    println!("Generated at Unix: {}", report.generated_at_unix);
    println!("Requested pair: {}", report.requested_pair);

    if let Some(agent) = &report.requested_agent {
        println!("Requested agent: {agent}");
    }
    if let Some(repository_root) = &report.repository_root {
        println!("Repository: {repository_root}");
    }
    if let Some(current_branch) = &report.current_branch {
        println!("Current: {current_branch}");
    }
    if let Some(worktree) = report.worktree {
        println!("Worktree: {worktree}");
    }
    if let Some(git_state) = report.git_state {
        println!("Git state: {git_state}");
    }

    if let Some(pair) = &report.pair {
        println!("Pair: {} ({} <-> {})", pair.pair, pair.left, pair.right);
        if let Some(other) = &pair.other {
            println!("Other: {other}");
        } else {
            println!("Other: unavailable");
        }

        if pair.switch_allowed {
            println!("Switch: allowed");
        } else {
            let reasons = pair
                .refusal_reasons
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ");
            println!("Switch: refused ({reasons})");
        }
    }

    if let Some(expectation) = &report.expectation {
        print_branch_expectation_report(expectation);
    }

    if let Some(claim) = &report.claim {
        print_claim_readiness("Claim", claim);
    }
    if let Some(target_claim) = &report.target_claim {
        print_claim_readiness("Target claim", target_claim);
    }

    if report.claims.is_empty() {
        println!("Claims: none");
    } else {
        println!("Claims:");
        for claim in &report.claims {
            println!("{}", format_claim_line(claim));
        }
    }

    for error in &report.errors {
        println!("Error: {} ({})", error.message, error.kind);
    }

    Ok(())
}

fn format_claim_line(claim: &AgentClaim) -> String {
    let mut line = format!(
        "- {}: {} on {} (created_at_unix: {})",
        claim.agent, claim.pair, claim.branch, claim.created_at_unix
    );
    if let Some(note) = &claim.note {
        line.push_str(" note: ");
        line.push_str(note);
    }
    line
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
    pair: BranchPair,
    status: PairStatus,
}

#[derive(Debug, Serialize)]
struct PairMutationReport {
    ok: bool,
    action: &'static str,
    repository_root: String,
    pairs_path: String,
    pair: BranchPair,
    previous_pair: Option<BranchPair>,
}

impl PairMutationReport {
    fn new(
        action: &'static str,
        repository: &GitRepository,
        store: &MetadataStore,
        pair: BranchPair,
        previous_pair: Option<BranchPair>,
    ) -> Self {
        Self {
            ok: true,
            action,
            repository_root: repository.root().display().to_string(),
            pairs_path: store.path().display().to_string(),
            pair,
            previous_pair,
        }
    }
}

#[derive(Debug, Serialize)]
struct SwitchReport {
    ok: bool,
    dry_run: bool,
    switched: bool,
    pair: String,
    repository_root: String,
    current: String,
    target: String,
    worktree: WorktreeStatus,
    git_state: GitState,
    refusal_reasons: Vec<RefusalReason>,
    target_claim: Option<PreflightClaimReport>,
}

impl SwitchReport {
    fn from_status_context(context: &StatusContext, dry_run: bool) -> Self {
        Self {
            ok: context.status.switch_allowed,
            dry_run,
            switched: false,
            pair: context.status.pair.clone(),
            repository_root: context.repository.root().display().to_string(),
            current: context.status.current.clone(),
            target: context.status.other.clone(),
            worktree: context.status.worktree,
            git_state: context.status.git_state,
            refusal_reasons: context.status.refusal_reasons.clone(),
            target_claim: None,
        }
    }
}

#[derive(Debug, Serialize)]
struct PreflightReport {
    ready: bool,
    pair: String,
    repository_root: Option<String>,
    current: Option<String>,
    other: Option<String>,
    worktree: Option<WorktreeStatus>,
    git_state: Option<GitState>,
    switch_allowed: bool,
    refusal_reasons: Vec<RefusalReason>,
    expectation: Option<BranchExpectationReport>,
    claim: Option<PreflightClaimReport>,
    target_claim: Option<PreflightClaimReport>,
    error: Option<PreflightErrorReport>,
}

impl PreflightReport {
    fn from_status_context(context: &StatusContext) -> Self {
        Self {
            ready: context.status.switch_allowed,
            pair: context.status.pair.clone(),
            repository_root: Some(context.repository.root().display().to_string()),
            current: Some(context.status.current.clone()),
            other: Some(context.status.other.clone()),
            worktree: Some(context.status.worktree),
            git_state: Some(context.status.git_state),
            switch_allowed: context.status.switch_allowed,
            refusal_reasons: context.status.refusal_reasons.clone(),
            expectation: None,
            claim: None,
            target_claim: None,
            error: None,
        }
    }

    fn from_error(pair: &str, error: &AppError) -> Self {
        Self {
            ready: false,
            pair: pair.to_owned(),
            repository_root: None,
            current: None,
            other: None,
            worktree: None,
            git_state: None,
            switch_allowed: false,
            refusal_reasons: Vec::new(),
            expectation: None,
            claim: None,
            target_claim: None,
            error: Some(PreflightErrorReport {
                kind: error.kind(),
                message: error.to_string(),
                exit_code: error.exit_code(),
            }),
        }
    }
}

#[derive(Debug, Serialize)]
struct PreflightErrorReport {
    kind: &'static str,
    message: String,
    exit_code: u8,
}

#[derive(Debug, Serialize)]
struct BranchExpectationReport {
    ok: bool,
    expected_branch: Option<String>,
    expected_side: Option<&'static str>,
    current_side: Option<&'static str>,
    failures: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PreflightClaimReport {
    requested_agent: String,
    claim_allowed: bool,
    claim_required: bool,
    claim_owned: bool,
    owned_claim: Option<AgentClaim>,
    metadata_lock: MetadataLockReport,
    stale_after_seconds: Option<u64>,
    conflict_stale: Option<bool>,
    conflict: Option<AgentClaim>,
}

#[derive(Debug, Serialize)]
struct AssertReport {
    ok: bool,
    repository_root: String,
    current_branch: String,
    expected_branch: Option<String>,
    pair: Option<AssertPairReport>,
    claim: Option<PreflightClaimReport>,
    failures: Vec<String>,
}

#[derive(Debug, Serialize)]
struct AssertPairReport {
    name: String,
    left: Option<String>,
    right: Option<String>,
    configured: bool,
    current_side: Option<&'static str>,
    expected_side: Option<&'static str>,
}

#[derive(Debug, Serialize)]
struct ClaimOperationReport {
    ok: bool,
    status: &'static str,
    repository_root: String,
    claims_path: String,
    agent: String,
    pair: String,
    branch: String,
    claim: Option<AgentClaim>,
    conflict: Option<AgentClaim>,
    expectation: Option<BranchExpectationReport>,
    refusal_reasons: Vec<RefusalReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stale_after_seconds: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    conflict_stale: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ClaimsReport {
    repository_root: String,
    claims_path: String,
    filters: ClaimsFilterReport,
    claims: Vec<AgentClaim>,
}

#[derive(Debug, Serialize)]
struct PruneClaimsReport {
    applied: bool,
    repository_root: String,
    claims_path: String,
    filters: ClaimsFilterReport,
    orphaned: bool,
    pruned_claims: Vec<AgentClaim>,
    pruned_claim_issues: Vec<DoctorClaimIssueReport>,
    remaining_claim_count: usize,
}

#[derive(Debug, Serialize)]
struct ClaimsFilterReport {
    agent: Option<String>,
    conflicts_for: Option<String>,
    pair: Option<String>,
    branch: Option<String>,
    current: bool,
    target: bool,
    side: Option<&'static str>,
    stale_after_seconds: Option<u64>,
}

#[derive(Debug, Serialize)]
struct HandoffReport {
    ok: bool,
    generated_at_unix: u64,
    requested_pair: String,
    requested_agent: Option<String>,
    repository_root: Option<String>,
    current_branch: Option<String>,
    worktree: Option<WorktreeStatus>,
    git_state: Option<GitState>,
    expectation: Option<BranchExpectationReport>,
    pair: Option<PairStatusReport>,
    claims: Vec<AgentClaim>,
    claim: Option<PreflightClaimReport>,
    target_claim: Option<PreflightClaimReport>,
    errors: Vec<HandoffErrorReport>,
}

#[derive(Debug, Serialize)]
struct HandoffErrorReport {
    kind: &'static str,
    message: String,
    exit_code: u8,
}

impl HandoffErrorReport {
    fn from_error(error: &AppError) -> Self {
        Self {
            kind: error.kind(),
            message: error.to_string(),
            exit_code: error.exit_code(),
        }
    }
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

#[derive(Debug, Serialize)]
struct DoctorReport {
    healthy: bool,
    git: DoctorGitReport,
    repository: DoctorRepositoryReport,
    current_branch: Option<DoctorCurrentBranchReport>,
    worktree: Option<DoctorWorktreeReport>,
    git_state: Option<String>,
    metadata_lock: Option<MetadataLockReport>,
    metadata: Option<DoctorMetadataReport>,
    claims: Option<DoctorClaimsReport>,
}

impl Default for DoctorReport {
    fn default() -> Self {
        Self {
            healthy: true,
            git: DoctorGitReport::default(),
            repository: DoctorRepositoryReport::default(),
            current_branch: None,
            worktree: None,
            git_state: None,
            metadata_lock: None,
            metadata: None,
            claims: None,
        }
    }
}

#[derive(Debug, Default, Serialize)]
struct DoctorGitReport {
    ok: bool,
    version: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Default, Serialize)]
struct DoctorRepositoryReport {
    ok: bool,
    root: Option<String>,
    git_dir: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct DoctorCurrentBranchReport {
    ok: bool,
    branch: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct DoctorWorktreeReport {
    ok: bool,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct MetadataLockReport {
    ok: bool,
    path: Option<String>,
    locked: Option<bool>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct DoctorMetadataReport {
    ok: bool,
    path: Option<String>,
    pair_count: Option<usize>,
    error: Option<String>,
    pairs: Vec<DoctorPairReport>,
}

#[derive(Debug, Serialize)]
struct DoctorPairReport {
    name: String,
    left: String,
    right: String,
    ok: bool,
    summary: String,
}

#[derive(Debug, Serialize)]
struct DoctorClaimsReport {
    ok: bool,
    path: Option<String>,
    claim_count: Option<usize>,
    claim_issue_count: Option<usize>,
    claim_issues: Vec<DoctorClaimIssueReport>,
    stale_after_seconds: Option<u64>,
    stale_claim_count: Option<usize>,
    stale_claims: Vec<AgentClaim>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct DoctorClaimIssueReport {
    claim: AgentClaim,
    reason: &'static str,
    message: String,
}

fn pair_side_for_branch(pair: &BranchPair, branch: &str) -> Option<PairSide> {
    if branch == pair.left {
        Some(PairSide::Left)
    } else if branch == pair.right {
        Some(PairSide::Right)
    } else {
        None
    }
}

fn resolve_pair_side_branch(
    repository: &GitRepository,
    pair_filter: &mut Option<String>,
    side: PairSide,
) -> Result<String, AppError> {
    let pair_name = match pair_filter {
        Some(pair) => pair.clone(),
        None => {
            let pair_name = "default".to_owned();
            *pair_filter = Some(pair_name.clone());
            pair_name
        }
    };
    let pair_store = MetadataStore::for_repository(repository);
    let pairs = pair_store.load()?;
    let pair = pairs
        .get(&pair_name)
        .ok_or(AppError::PairNotFound { name: pair_name })?;

    Ok(match side {
        PairSide::Left => pair.left.clone(),
        PairSide::Right => pair.right.clone(),
    })
}

fn pair_side_name(side: PairSide) -> &'static str {
    match side {
        PairSide::Left => "left",
        PairSide::Right => "right",
    }
}

fn current_unix_timestamp() -> Result<u64, AppError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

fn claim_is_stale(claim: &AgentClaim, now_unix: u64, stale_after_seconds: u64) -> bool {
    now_unix.saturating_sub(claim.created_at_unix) >= stale_after_seconds
}

fn claim_matches_filters(
    claim: &AgentClaim,
    agent: Option<&str>,
    pair: Option<&str>,
    branch: Option<&str>,
) -> bool {
    agent.is_none_or(|agent| claim.agent == agent)
        && pair.is_none_or(|pair| claim.pair == pair)
        && branch.is_none_or(|branch| claim.branch == branch)
}

fn same_claim_scope(left: &AgentClaim, right: &AgentClaim) -> bool {
    left.agent == right.agent && left.pair == right.pair && left.branch == right.branch
}

fn parse_duration_seconds(value: &str) -> Result<u64, AppError> {
    let digits_end = value
        .char_indices()
        .find_map(|(index, character)| (!character.is_ascii_digit()).then_some(index))
        .unwrap_or(value.len());
    let (amount, unit) = value.split_at(digits_end);

    if amount.is_empty() {
        return Err(AppError::InvalidDuration {
            value: value.to_owned(),
        });
    }

    let amount = amount
        .parse::<u64>()
        .map_err(|_| AppError::InvalidDuration {
            value: value.to_owned(),
        })?;
    if amount == 0 {
        return Err(AppError::InvalidDuration {
            value: value.to_owned(),
        });
    }

    let multiplier = match unit {
        "" | "s" => 1,
        "m" => 60,
        "h" => 60 * 60,
        "d" => 24 * 60 * 60,
        _ => {
            return Err(AppError::InvalidDuration {
                value: value.to_owned(),
            });
        }
    };

    amount
        .checked_mul(multiplier)
        .ok_or_else(|| AppError::InvalidDuration {
            value: value.to_owned(),
        })
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
    AssertFailed {
        failures: Vec<String>,
    },
    BranchNotFound {
        branch: String,
    },
    Claim {
        source: ClaimError,
    },
    ClaimBlocked {
        agent: String,
        pair: String,
        branch: String,
        reason: String,
    },
    ClaimConflict {
        agent: String,
        pair: String,
        branch: String,
    },
    ClaimNotFound {
        agent: String,
        pair: String,
        branch: String,
    },
    ClaimRequired {
        agent: String,
        pair: String,
        branch: String,
    },
    Clock {
        source: SystemTimeError,
    },
    DoctorFailed,
    Git {
        source: GitError,
    },
    InvalidBranchName {
        branch: String,
    },
    InvalidDuration {
        value: String,
    },
    Metadata {
        source: MetadataError,
    },
    Pair {
        source: PairError,
    },
    PairAlreadyExists {
        name: String,
    },
    PairNotFound {
        name: String,
    },
    PreflightRefused {
        reasons: Vec<RefusalReason>,
    },
    Serialize {
        source: serde_json::Error,
    },
    Status {
        source: StatusError,
    },
    SwitchRefused {
        reasons: Vec<RefusalReason>,
    },
}

impl Display for AppError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::AssertFailed { failures } => {
                write!(formatter, "assertion failed: {}", failures.join("; "))
            }
            Self::BranchNotFound { branch } => write!(formatter, "branch '{branch}' was not found"),
            Self::Claim { source } => Display::fmt(source, formatter),
            Self::ClaimBlocked {
                agent,
                pair,
                branch,
                reason,
            } => write!(
                formatter,
                "claim for agent '{agent}' on pair '{pair}' and branch '{branch}' is blocked: {reason}"
            ),
            Self::ClaimConflict {
                agent,
                pair,
                branch,
            } => write!(
                formatter,
                "pair '{pair}' on branch '{branch}' is already claimed by agent '{agent}'"
            ),
            Self::ClaimNotFound {
                agent,
                pair,
                branch,
            } => write!(
                formatter,
                "no claim for agent '{agent}' on pair '{pair}' and branch '{branch}'"
            ),
            Self::ClaimRequired {
                agent,
                pair,
                branch,
            } => write!(
                formatter,
                "required claim for agent '{agent}' on pair '{pair}' and branch '{branch}' was not found"
            ),
            Self::Clock { source } => Display::fmt(source, formatter),
            Self::DoctorFailed => write!(formatter, "doctor found problems"),
            Self::Git { source } => Display::fmt(source, formatter),
            Self::InvalidBranchName { branch } => {
                write!(formatter, "branch name '{branch}' is invalid")
            }
            Self::InvalidDuration { value } => write!(
                formatter,
                "duration '{value}' is invalid; use a positive number followed by s, m, h, or d"
            ),
            Self::Metadata { source } => Display::fmt(source, formatter),
            Self::Pair { source } => Display::fmt(source, formatter),
            Self::PairAlreadyExists { name } => write!(formatter, "pair '{name}' already exists"),
            Self::PairNotFound { name } => write!(formatter, "pair '{name}' was not found"),
            Self::PreflightRefused { reasons } => {
                let reasons = reasons
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ");
                write!(formatter, "preflight failed: {reasons}")
            }
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
            Self::Claim { source } => Some(source),
            Self::Clock { source } => Some(source),
            Self::Git { source } => Some(source),
            Self::Metadata { source } => Some(source),
            Self::Pair { source } => Some(source),
            Self::Serialize { source } => Some(source),
            Self::Status { source } => Some(source),
            Self::AssertFailed { .. }
            | Self::BranchNotFound { .. }
            | Self::ClaimBlocked { .. }
            | Self::ClaimConflict { .. }
            | Self::ClaimNotFound { .. }
            | Self::ClaimRequired { .. }
            | Self::DoctorFailed
            | Self::InvalidBranchName { .. }
            | Self::InvalidDuration { .. }
            | Self::PairAlreadyExists { .. }
            | Self::PairNotFound { .. }
            | Self::PreflightRefused { .. }
            | Self::SwitchRefused { .. } => None,
        }
    }
}

impl AppError {
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::AssertFailed { .. }
            | Self::BranchNotFound { .. }
            | Self::Claim { .. }
            | Self::ClaimNotFound { .. }
            | Self::InvalidBranchName { .. }
            | Self::InvalidDuration { .. }
            | Self::Pair { .. }
            | Self::PairAlreadyExists { .. }
            | Self::PairNotFound { .. }
            | Self::Status { .. } => 2,
            Self::ClaimConflict { .. }
            | Self::ClaimBlocked { .. }
            | Self::ClaimRequired { .. }
            | Self::PreflightRefused { .. }
            | Self::SwitchRefused { .. } => 3,
            Self::DoctorFailed => 4,
            Self::Git {
                source: GitError::DetachedHead | GitError::NotRepository,
            } => 2,
            Self::Clock { .. }
            | Self::Git { .. }
            | Self::Metadata { .. }
            | Self::Serialize { .. } => 1,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::AssertFailed { .. } => "assert_failed",
            Self::BranchNotFound { .. } => "branch_not_found",
            Self::Claim { .. } => "claim_error",
            Self::ClaimBlocked { .. } => "claim_blocked",
            Self::ClaimConflict { .. } => "claim_conflict",
            Self::ClaimNotFound { .. } => "claim_not_found",
            Self::ClaimRequired { .. } => "claim_required",
            Self::Clock { .. } => "clock_error",
            Self::DoctorFailed => "doctor_failed",
            Self::Git { source } => source.kind(),
            Self::InvalidBranchName { .. } => "invalid_branch_name",
            Self::InvalidDuration { .. } => "invalid_duration",
            Self::Metadata { .. } => "metadata_error",
            Self::Pair { .. } => "pair_error",
            Self::PairAlreadyExists { .. } => "pair_already_exists",
            Self::PairNotFound { .. } => "pair_not_found",
            Self::PreflightRefused { .. } => "preflight_refused",
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

impl From<ClaimError> for AppError {
    fn from(source: ClaimError) -> Self {
        Self::Claim { source }
    }
}

impl From<SystemTimeError> for AppError {
    fn from(source: SystemTimeError) -> Self {
        Self::Clock { source }
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
