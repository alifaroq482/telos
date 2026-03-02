use anyhow::{Context, Result};
use std::collections::HashSet;
use std::env;
use std::path::Path;
use telos_core::hash::ObjectId;
use telos_core::object::constraint::Constraint;
use telos_core::object::decision_record::DecisionRecord;
use telos_core::object::intent::Intent;
use telos_core::object::TelosObject;
use telos_store::query;
use telos_store::repository::Repository;

use crate::render;

struct ConstraintHit {
    id: ObjectId,
    constraint: Constraint,
    symbol: Option<String>,
    binding_type: String,
}

pub fn run(path: Option<String>, commit: Option<String>, json: bool) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd).context("not a Telos repository")?;

    if let Some(commit_sha) = commit {
        return run_commit(&repo, &commit_sha, json);
    }

    let target = path.context("provide a file path or --commit <sha>")?;
    let repo_root = repo.root().to_path_buf();
    let rel_path = normalize_path(&cwd, &repo_root, &target);

    let abs_path = repo_root.join(&rel_path);
    if abs_path.is_dir() || target.ends_with('/') {
        return run_directory(&repo, &rel_path, json);
    }

    run_file(&repo, &rel_path, json)
}

fn normalize_path(cwd: &Path, repo_root: &Path, path: &str) -> String {
    let p = Path::new(path);
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    };
    match abs.strip_prefix(repo_root) {
        Ok(rel) => rel.to_string_lossy().to_string(),
        Err(_) => path.to_string(),
    }
}

fn collect_file_hits(repo: &Repository, path: &str) -> Result<Vec<ConstraintHit>> {
    let entries = repo.indexes.by_path(path);
    let mut hits = Vec::new();
    let mut seen = HashSet::new();
    for entry in &entries {
        if let Ok(binding_id) = ObjectId::parse(&entry.id) {
            if let Ok(TelosObject::CodeBinding(cb)) = repo.odb.read(&binding_id) {
                let bound_id = &cb.bound_object;
                if seen.insert(bound_id.hex().to_string()) {
                    if let Ok(TelosObject::Constraint(c)) = repo.odb.read(bound_id) {
                        hits.push(ConstraintHit {
                            id: bound_id.clone(),
                            constraint: c,
                            symbol: cb.symbol.clone(),
                            binding_type: entry
                                .binding_type
                                .clone()
                                .unwrap_or_else(|| "file".into()),
                        });
                    }
                }
            }
        }
    }
    Ok(hits)
}

fn run_file(repo: &Repository, path: &str, json: bool) -> Result<()> {
    let hits = collect_file_hits(repo, path)?;
    let (intents, decisions) = collect_intents_and_decisions(repo, &hits)?;

    if json {
        return print_json(path, &hits, &intents, &decisions);
    }
    print_impact(path, &hits, &intents, &decisions);
    Ok(())
}

fn run_directory(repo: &Repository, dir: &str, json: bool) -> Result<()> {
    let output = std::process::Command::new("git")
        .args(["ls-files", "--", dir])
        .current_dir(repo.root())
        .output()
        .context("failed to run git ls-files")?;

    let files: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect();

    let mut all_hits = Vec::new();
    let mut seen = HashSet::new();
    for file in &files {
        let hits = collect_file_hits(repo, file)?;
        for hit in hits {
            if seen.insert(hit.id.hex().to_string()) {
                all_hits.push(hit);
            }
        }
    }

    let (intents, decisions) = collect_intents_and_decisions(repo, &all_hits)?;
    let label = format!("{}/ ({} files)", dir.trim_end_matches('/'), files.len());

    if json {
        return print_json(&label, &all_hits, &intents, &decisions);
    }
    print_impact(&label, &all_hits, &intents, &decisions);
    Ok(())
}

fn run_commit(repo: &Repository, commit_sha: &str, json: bool) -> Result<()> {
    let changesets =
        query::query_changesets(&repo.odb, &repo.indexes, Some(commit_sha), None)?;

    if changesets.is_empty() {
        println!("No changeset found for commit {}", commit_sha);
        return Ok(());
    }

    let mut hits = Vec::new();
    let mut intents = Vec::new();
    let mut decisions = Vec::new();
    let mut seen = HashSet::new();

    for (_, cs) in &changesets {
        for cid in &cs.constraints {
            if seen.insert(cid.hex().to_string()) {
                if let Ok(TelosObject::Constraint(c)) = repo.odb.read(cid) {
                    hits.push(ConstraintHit {
                        id: cid.clone(),
                        constraint: c,
                        symbol: None,
                        binding_type: "changeset".to_string(),
                    });
                }
            }
        }
        for iid in &cs.intents {
            if seen.insert(iid.hex().to_string()) {
                if let Ok(TelosObject::Intent(i)) = repo.odb.read(iid) {
                    intents.push((iid.clone(), i));
                }
            }
        }
        for did in &cs.decisions {
            if seen.insert(did.hex().to_string()) {
                if let Ok(TelosObject::DecisionRecord(d)) = repo.odb.read(did) {
                    decisions.push((did.clone(), d));
                }
            }
        }
    }

    let label = format!("commit {}", commit_sha);
    if json {
        return print_json(&label, &hits, &intents, &decisions);
    }
    print_impact(&label, &hits, &intents, &decisions);
    Ok(())
}

fn collect_intents_and_decisions(
    repo: &Repository,
    hits: &[ConstraintHit],
) -> Result<(Vec<(ObjectId, Intent)>, Vec<(ObjectId, DecisionRecord)>)> {
    let mut intents = Vec::new();
    let mut seen_intents = HashSet::new();
    for hit in hits {
        let intent_id = &hit.constraint.source_intent;
        if seen_intents.insert(intent_id.hex().to_string()) {
            if let Ok(TelosObject::Intent(intent)) = repo.odb.read(intent_id) {
                intents.push((intent_id.clone(), intent));
            }
        }
    }

    let mut decisions = Vec::new();
    for (intent_id, _) in &intents {
        let ds = query::query_decisions(&repo.odb, Some(intent_id), None)?;
        for d in ds {
            decisions.push(d);
        }
    }

    Ok((intents, decisions))
}

fn print_impact(
    target: &str,
    constraints: &[ConstraintHit],
    intents: &[(ObjectId, Intent)],
    decisions: &[(ObjectId, DecisionRecord)],
) {
    println!("Impact: {}", target);
    println!("{}", "─".repeat(target.len() + 8));
    println!();

    if constraints.is_empty() && intents.is_empty() && decisions.is_empty() {
        println!("  No constraints or bindings found.");
        return;
    }

    if !constraints.is_empty() {
        println!("Constraints ({}):", constraints.len());
        for hit in constraints {
            let sev = render::colored_severity(&hit.constraint.severity);
            let bullet = render::status_bullet(&hit.constraint.status);
            println!("  {} {} \"{}\"", sev, bullet, hit.constraint.statement);
            let bound_to = match (&hit.symbol, hit.binding_type.as_str()) {
                (Some(sym), "function") => format!("{}()", sym),
                (Some(sym), _) => sym.clone(),
                (None, _) => "file".to_string(),
            };
            println!(
                "             constraint {} · bound to {}",
                render::short_id(&hit.id),
                bound_to
            );
        }
        println!();
    }

    if !intents.is_empty() {
        println!("Intents ({}):", intents.len());
        for (id, intent) in intents {
            let decision_count = decisions.iter().filter(|(_, d)| d.intent_id == *id).count();
            println!(
                "  intent {} \"{}\"",
                render::short_id(id),
                intent.statement
            );
            if decision_count > 0 {
                println!(
                    "  └─ {} decision{} recorded",
                    decision_count,
                    if decision_count == 1 { "" } else { "s" }
                );
            }
        }
        println!();
    }

    if !decisions.is_empty() {
        println!("Decisions ({}):", decisions.len());
        for (id, dr) in decisions {
            println!("  decision {} \"{}\"", render::short_id(id), dr.decision);
        }
    }
}

fn print_json(
    target: &str,
    constraints: &[ConstraintHit],
    intents: &[(ObjectId, Intent)],
    decisions: &[(ObjectId, DecisionRecord)],
) -> Result<()> {
    let constraint_json: Vec<_> = constraints
        .iter()
        .map(|hit| {
            serde_json::json!({
                "id": hit.id.hex(),
                "statement": hit.constraint.statement,
                "severity": format!("{:?}", hit.constraint.severity),
                "status": format!("{:?}", hit.constraint.status),
                "binding_symbol": hit.symbol,
                "binding_type": hit.binding_type,
            })
        })
        .collect();

    let intent_json: Vec<_> = intents
        .iter()
        .map(|(id, i)| {
            serde_json::json!({
                "id": id.hex(),
                "statement": i.statement,
                "impacts": i.impacts,
            })
        })
        .collect();

    let decision_json: Vec<_> = decisions
        .iter()
        .map(|(id, d)| {
            serde_json::json!({
                "id": id.hex(),
                "question": d.question,
                "decision": d.decision,
                "rationale": d.rationale,
            })
        })
        .collect();

    let output = serde_json::json!({
        "target": target,
        "constraints": constraint_json,
        "intents": intent_json,
        "decisions": decision_json,
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
