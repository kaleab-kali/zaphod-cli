use crate::cli::{Cli, CliCommand, PairSide};
use crate::core::{
    AgentClaim, BranchPair, ClaimError, GitState, PairError, PairStatus, RefusalReason,
    StatusError, WorktreeStatus, validate_agent_name,
};
use crate::git::{GitError, GitRepository};
use crate::metadata::{ClaimStore, MetadataError, MetadataStore};
use clap::CommandFactory;
use clap_complete::Shell;
use serde::Serialize;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io;
use std::time::{SystemTime, SystemTimeError, UNIX_EPOCH};

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
        CliCommand::Switch { dry_run, name } => switch_branches(&name, dry_run),
        CliCommand::Preflight { json, name, agent } => run_preflight(&name, json, agent),
        CliCommand::Assert {
            json,
            pair,
            branch,
            side,
        } => assert_repository_state(json, pair, branch, side),
        CliCommand::Claim { json, agent, pair } => claim_current_scope(json, agent, pair),
        CliCommand::Claims { json } => list_claims(json),
        CliCommand::Unclaim { json, agent, pair } => unclaim_current_scope(json, agent, pair),
        CliCommand::Handoff { json, name, agent } => show_handoff(json, name, agent),
        CliCommand::Doctor { json } => run_doctor(json),
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

fn switch_branches(name: &str, dry_run: bool) -> Result<(), AppError> {
    let context = load_status_context(name)?;

    if !context.status.switch_allowed {
        return Err(AppError::SwitchRefused {
            reasons: context.status.refusal_reasons,
        });
    }

    if dry_run {
        println!(
            "Would switch pair '{}': {} -> {}",
            context.status.pair, context.status.current, context.status.other
        );
        return Ok(());
    }

    context.repository.switch_branch(&context.status.other)?;
    println!(
        "Switched pair '{}': {} -> {}",
        context.status.pair, context.status.current, context.status.other
    );

    Ok(())
}

fn run_preflight(name: &str, json: bool, agent: Option<String>) -> Result<(), AppError> {
    if let Some(agent) = &agent {
        validate_agent_name(agent)?;
    }

    match load_status_context(name) {
        Ok(context) => {
            let mut report = PreflightReport::from_status_context(&context);
            let claim_conflict = if let Some(agent) = agent {
                let claim_report = build_preflight_claim_report(&context, agent)?;
                let conflict = claim_report.conflict.clone();
                report.ready = report.ready && claim_report.claim_allowed;
                report.claim = Some(claim_report);
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
            } else if context.status.switch_allowed {
                Ok(())
            } else {
                Err(AppError::PreflightRefused {
                    reasons: context.status.refusal_reasons,
                })
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

fn build_preflight_claim_report(
    context: &StatusContext,
    agent: String,
) -> Result<PreflightClaimReport, AppError> {
    let store = ClaimStore::for_repository(&context.repository);
    let claims = store.load()?;
    let conflict = claims
        .conflict_for_scope(&agent, &context.status.pair, &context.status.current)
        .cloned();

    Ok(PreflightClaimReport {
        requested_agent: agent,
        claim_allowed: conflict.is_none(),
        conflict,
    })
}

fn assert_repository_state(
    json: bool,
    pair_name: Option<String>,
    expected_branch: Option<String>,
    expected_side: Option<PairSide>,
) -> Result<(), AppError> {
    let repository = GitRepository::discover(".")?;
    let current_branch = repository.current_branch()?;
    let effective_pair = pair_name
        .or_else(|| expected_side.map(|_| "default".to_owned()))
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

    let report = AssertReport {
        ok: failures.is_empty(),
        repository_root: repository.root().display().to_string(),
        current_branch,
        expected_branch,
        pair: pair_report,
        failures,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_assert_report(&report);
    }

    if report.ok {
        Ok(())
    } else {
        Err(AppError::AssertFailed {
            failures: report.failures,
        })
    }
}

fn claim_current_scope(json: bool, agent: String, pair_name: String) -> Result<(), AppError> {
    validate_agent_name(&agent)?;
    let context = load_status_context(&pair_name)?;
    let store = ClaimStore::for_repository(&context.repository);

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
            refusal_reasons: context.status.refusal_reasons.clone(),
        };
        print_claim_operation_report(&report, json)?;
        return Err(AppError::PreflightRefused {
            reasons: context.status.refusal_reasons,
        });
    }

    let mut claims = store.load()?;
    if let Some(conflict) =
        claims.conflict_for_scope(&agent, &context.status.pair, &context.status.current)
    {
        let conflict_agent = conflict.agent.clone();
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
            refusal_reasons: Vec::new(),
        };
        print_claim_operation_report(&report, json)?;
        return Err(AppError::ClaimConflict {
            agent: conflict_agent,
            pair: report.pair,
            branch: report.branch,
        });
    }

    let claim = AgentClaim::new(
        agent,
        context.status.pair,
        context.status.current,
        current_unix_timestamp()?,
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
        refusal_reasons: Vec::new(),
    };
    print_claim_operation_report(&report, json)?;

    Ok(())
}

fn list_claims(json: bool) -> Result<(), AppError> {
    let repository = GitRepository::discover(".")?;
    let store = ClaimStore::for_repository(&repository);
    let claims = store.load()?;
    let report = ClaimsReport {
        repository_root: repository.root().display().to_string(),
        claims_path: store.path().display().to_string(),
        claims: claims.claims().to_vec(),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    print_claims_report(&report);
    Ok(())
}

fn unclaim_current_scope(json: bool, agent: String, pair_name: String) -> Result<(), AppError> {
    validate_agent_name(&agent)?;
    let repository = GitRepository::discover(".")?;
    let current_branch = repository.current_branch()?;
    let store = ClaimStore::for_repository(&repository);
    let mut claims = store.load()?;

    let removed = claims
        .remove(&agent, &pair_name, &current_branch)
        .ok_or_else(|| AppError::ClaimNotFound {
            agent: agent.clone(),
            pair: pair_name.clone(),
            branch: current_branch.clone(),
        })?;
    store.save(&claims)?;

    let report = ClaimOperationReport {
        ok: true,
        status: "released",
        repository_root: repository.root().display().to_string(),
        claims_path: store.path().display().to_string(),
        agent,
        pair: pair_name,
        branch: current_branch,
        claim: Some(removed),
        conflict: None,
        refusal_reasons: Vec::new(),
    };
    print_claim_operation_report(&report, json)?;

    Ok(())
}

fn show_handoff(json: bool, pair_name: String, agent: Option<String>) -> Result<(), AppError> {
    if let Some(agent) = &agent {
        validate_agent_name(agent)?;
    }

    let mut report = HandoffReport {
        ok: false,
        generated_at_unix: current_unix_timestamp()?,
        requested_pair: pair_name.clone(),
        requested_agent: agent,
        repository_root: None,
        current_branch: None,
        worktree: None,
        git_state: None,
        pair: None,
        claims: Vec::new(),
        claim: None,
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

    if let Some(agent) = report.requested_agent.clone() {
        let conflict = claims
            .conflict_for_scope(&agent, &pair_report.pair, &pair_report.current)
            .cloned();
        report.claim = Some(PreflightClaimReport {
            requested_agent: agent,
            claim_allowed: conflict.is_none(),
            conflict,
        });
    }

    report.pair = Some(pair_report);
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

fn run_doctor(json: bool) -> Result<(), AppError> {
    let report = build_doctor_report();

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

fn build_doctor_report() -> DoctorReport {
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

    if let Some(claim) = &report.claim {
        if claim.claim_allowed {
            println!("Claim: allowed for {}", claim.requested_agent);
        } else if let Some(conflict) = &claim.conflict {
            println!("Claim: refused (claimed by {})", conflict.agent);
        }
    }

    if let Some(error) = &report.error {
        println!("Error: {}", error.message);
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

    for failure in &report.failures {
        println!("Failure: {failure}");
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

    if let Some(conflict) = &report.conflict {
        println!("Conflict: claimed by {}", conflict.agent);
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

    if report.claims.is_empty() {
        println!("No active agent claims.");
        return;
    }

    println!("Claims:");
    for claim in &report.claims {
        println!(
            "- {}: {} on {} (created_at_unix: {})",
            claim.agent, claim.pair, claim.branch, claim.created_at_unix
        );
    }
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

    if let Some(claim) = &report.claim {
        if claim.claim_allowed {
            println!("Claim: allowed for {}", claim.requested_agent);
        } else if let Some(conflict) = &claim.conflict {
            println!("Claim: refused (claimed by {})", conflict.agent);
        }
    }

    if report.claims.is_empty() {
        println!("Claims: none");
    } else {
        println!("Claims:");
        for claim in &report.claims {
            println!(
                "- {}: {} on {} (created_at_unix: {})",
                claim.agent, claim.pair, claim.branch, claim.created_at_unix
            );
        }
    }

    for error in &report.errors {
        println!("Error: {} ({})", error.message, error.kind);
    }

    Ok(())
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
    claim: Option<PreflightClaimReport>,
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
            claim: None,
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
            claim: None,
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
struct PreflightClaimReport {
    requested_agent: String,
    claim_allowed: bool,
    conflict: Option<AgentClaim>,
}

#[derive(Debug, Serialize)]
struct AssertReport {
    ok: bool,
    repository_root: String,
    current_branch: String,
    expected_branch: Option<String>,
    pair: Option<AssertPairReport>,
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
    refusal_reasons: Vec<RefusalReason>,
}

#[derive(Debug, Serialize)]
struct ClaimsReport {
    repository_root: String,
    claims_path: String,
    claims: Vec<AgentClaim>,
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
    pair: Option<PairStatusReport>,
    claims: Vec<AgentClaim>,
    claim: Option<PreflightClaimReport>,
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
    metadata: Option<DoctorMetadataReport>,
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
            metadata: None,
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

fn pair_side_for_branch(pair: &BranchPair, branch: &str) -> Option<PairSide> {
    if branch == pair.left {
        Some(PairSide::Left)
    } else if branch == pair.right {
        Some(PairSide::Right)
    } else {
        None
    }
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
            Self::Clock { source } => Display::fmt(source, formatter),
            Self::DoctorFailed => write!(formatter, "doctor found problems"),
            Self::Git { source } => Display::fmt(source, formatter),
            Self::InvalidBranchName { branch } => {
                write!(formatter, "branch name '{branch}' is invalid")
            }
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
            | Self::ClaimConflict { .. }
            | Self::ClaimNotFound { .. }
            | Self::DoctorFailed
            | Self::InvalidBranchName { .. }
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
            | Self::Pair { .. }
            | Self::PairAlreadyExists { .. }
            | Self::PairNotFound { .. }
            | Self::Status { .. } => 2,
            Self::ClaimConflict { .. }
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
            Self::ClaimConflict { .. } => "claim_conflict",
            Self::ClaimNotFound { .. } => "claim_not_found",
            Self::Clock { .. } => "clock_error",
            Self::DoctorFailed => "doctor_failed",
            Self::Git { source } => source.kind(),
            Self::InvalidBranchName { .. } => "invalid_branch_name",
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
