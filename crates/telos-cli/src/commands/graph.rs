use anyhow::{Context, Result};
use std::collections::HashMap;
use std::env;
use telos_core::hash::ObjectId;
use telos_core::object::code_binding::CodeBinding;
use telos_core::object::constraint::{Constraint, ConstraintStatus};
use telos_core::object::intent::Intent;
use telos_core::object::TelosObject;
use telos_store::query;
use telos_store::repository::Repository;

use crate::render;

pub fn run(
    impact: Option<String>,
    id: Option<String>,
    depth: Option<usize>,
    json: bool,
) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd).context("not a Telos repository")?;

    // Pre-load all constraints and bindings for efficient lookup
    let all_objects = repo.odb.iter_all()?;
    let mut constraints_by_intent: HashMap<String, Vec<(ObjectId, Constraint)>> = HashMap::new();
    let mut bindings_by_object: HashMap<String, Vec<CodeBinding>> = HashMap::new();

    for (id, obj) in all_objects {
        match obj {
            TelosObject::Constraint(c) => {
                constraints_by_intent
                    .entry(c.source_intent.hex().to_string())
                    .or_default()
                    .push((id, c));
            }
            TelosObject::CodeBinding(cb) => {
                bindings_by_object
                    .entry(cb.bound_object.hex().to_string())
                    .or_default()
                    .push(cb);
            }
            _ => {}
        }
    }

    // Get intents
    let intents = if let Some(ref id_prefix) = id {
        let (oid, obj) = repo
            .read_object(id_prefix)
            .context(format!("object '{}' not found", id_prefix))?;
        match obj {
            TelosObject::Intent(i) => vec![(oid, i)],
            _ => anyhow::bail!("object {} is not an intent", oid.short()),
        }
    } else {
        query::query_intents(&repo.odb, impact.as_deref(), None)?
    };

    if intents.is_empty() {
        println!("No intents found.");
        return Ok(());
    }

    let max_depth = depth.unwrap_or(usize::MAX);

    if json {
        return print_json(
            &intents,
            &constraints_by_intent,
            &bindings_by_object,
            &repo,
            max_depth,
        );
    }

    for (intent_id, intent) in &intents {
        println!(
            "Intent {} \"{}\"",
            render::short_id(intent_id),
            intent.statement
        );
        println!(
            "│ {} · {} · impacts: {}",
            intent.author.email,
            intent.timestamp.format("%Y-%m-%d"),
            if intent.impacts.is_empty() {
                "none".to_string()
            } else {
                intent.impacts.join(", ")
            }
        );
        println!("│");

        if max_depth < 1 {
            println!();
            continue;
        }

        let intent_constraints = constraints_by_intent
            .get(intent_id.hex())
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        let intent_decisions = query::query_decisions(&repo.odb, Some(intent_id), None)
            .unwrap_or_default();

        let total_children = intent_constraints.len() + intent_decisions.len();
        let mut child_idx = 0;

        for (cid, constraint) in intent_constraints {
            child_idx += 1;
            let is_last = child_idx == total_children;
            let prefix = render::tree_prefix(&[is_last]);
            let cont = render::tree_continuation(&[is_last]);

            let sev = render::colored_severity(&constraint.severity);
            let bullet = render::status_bullet(&constraint.status);
            let status_tag = match constraint.status {
                ConstraintStatus::Active => String::new(),
                ConstraintStatus::Superseded => "  (superseded)".to_string(),
                ConstraintStatus::Deprecated => "  (deprecated)".to_string(),
            };

            println!(
                "{}{} {} constraint {}{}",
                prefix,
                sev,
                bullet,
                render::short_id(cid),
                status_tag
            );
            println!("{}\"{}\"", cont, constraint.statement);

            if max_depth >= 2 {
                if constraint.status == ConstraintStatus::Superseded {
                    if let Some(ref sup_id) = constraint.superseded_by {
                        println!(
                            "{}└─ superseded by {}",
                            cont,
                            render::short_id(sup_id)
                        );
                    }
                } else {
                    let constraint_bindings = bindings_by_object
                        .get(cid.hex())
                        .map(|v| v.as_slice())
                        .unwrap_or(&[]);

                    for (bi, binding) in constraint_bindings.iter().enumerate() {
                        let b_is_last = bi == constraint_bindings.len() - 1;
                        let b_prefix = if b_is_last { "└─" } else { "├─" };
                        let sym_display = match &binding.symbol {
                            Some(s) => format!(" :: {}", s),
                            None => String::new(),
                        };
                        println!("{}{} {}{}", cont, b_prefix, binding.path, sym_display);
                    }
                }
            }

            if !is_last {
                println!("│");
            }
        }

        for (did, decision) in &intent_decisions {
            child_idx += 1;
            let is_last = child_idx == total_children;
            let prefix = render::tree_prefix(&[is_last]);
            let cont = render::tree_continuation(&[is_last]);

            println!("{}decision {}", prefix, render::short_id(did));
            println!("{}Q: {}", cont, decision.question);
            println!("{}A: {}", cont, decision.decision);
            if !decision.alternatives.is_empty() {
                let rejected: Vec<_> = decision
                    .alternatives
                    .iter()
                    .map(|a| a.description.clone())
                    .collect();
                println!("{}Rejected: {}", cont, rejected.join(", "));
            }
        }

        println!();
    }

    Ok(())
}

fn print_json(
    intents: &[(ObjectId, Intent)],
    constraints_by_intent: &HashMap<String, Vec<(ObjectId, Constraint)>>,
    bindings_by_object: &HashMap<String, Vec<CodeBinding>>,
    repo: &Repository,
    max_depth: usize,
) -> Result<()> {
    let mut intent_json = Vec::new();

    for (intent_id, intent) in intents {
        let mut entry = serde_json::json!({
            "id": intent_id.hex(),
            "statement": intent.statement,
            "author": format!("{} <{}>", intent.author.name, intent.author.email),
            "timestamp": intent.timestamp.to_rfc3339(),
            "impacts": intent.impacts,
        });

        if max_depth >= 1 {
            let empty = vec![];
            let constraints: Vec<_> = constraints_by_intent
                .get(intent_id.hex())
                .unwrap_or(&empty)
                .iter()
                .map(|(cid, c)| {
                    let bindings: Vec<_> = if max_depth >= 2 {
                        bindings_by_object
                            .get(cid.hex())
                            .unwrap_or(&vec![])
                            .iter()
                            .map(|cb| {
                                serde_json::json!({
                                    "path": cb.path,
                                    "symbol": cb.symbol,
                                })
                            })
                            .collect()
                    } else {
                        vec![]
                    };
                    serde_json::json!({
                        "id": cid.hex(),
                        "statement": c.statement,
                        "severity": format!("{:?}", c.severity),
                        "status": format!("{:?}", c.status),
                        "bindings": bindings,
                    })
                })
                .collect();

            let decisions: Vec<_> = query::query_decisions(&repo.odb, Some(intent_id), None)
                .unwrap_or_default()
                .into_iter()
                .map(|(did, d)| {
                    serde_json::json!({
                        "id": did.hex(),
                        "question": d.question,
                        "decision": d.decision,
                    })
                })
                .collect();

            entry["constraints"] = serde_json::json!(constraints);
            entry["decisions"] = serde_json::json!(decisions);
        }

        intent_json.push(entry);
    }

    let output = serde_json::json!({ "intents": intent_json });
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
