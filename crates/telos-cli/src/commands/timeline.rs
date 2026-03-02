use anyhow::{Context, Result};
use std::collections::HashMap;
use std::env;
use telos_core::hash::ObjectId;
use telos_core::object::constraint::{Constraint, ConstraintStatus};
use telos_core::object::TelosObject;
use telos_store::repository::Repository;

use crate::render;

pub fn run(impact: Option<String>, json: bool) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd).context("not a Telos repository")?;

    // Collect all constraints (all statuses)
    let all = repo.odb.iter_all()?;
    let mut constraints: Vec<(ObjectId, Constraint)> = all
        .into_iter()
        .filter_map(|(id, obj)| match obj {
            TelosObject::Constraint(c) => Some((id, c)),
            _ => None,
        })
        .collect();

    // Filter by impact if specified
    if let Some(ref tag) = impact {
        constraints.retain(|(_, c)| c.impacts.iter().any(|i| i == tag));
    }

    // Sort by timestamp ascending (oldest first for timeline)
    constraints.sort_by(|a, b| a.1.timestamp.cmp(&b.1.timestamp));

    if constraints.is_empty() {
        println!("No constraints found.");
        return Ok(());
    }

    // Build a lookup map for resolving superseded_by references
    let constraint_map: HashMap<String, &Constraint> = constraints
        .iter()
        .map(|(id, c)| (id.hex().to_string(), c))
        .collect();

    if json {
        return print_json(&constraints);
    }

    println!("Constraint Timeline");
    println!("───────────────────");
    println!();

    for (id, c) in &constraints {
        println!(
            "{} {} \"{}\"",
            render::short_id(id),
            render::colored_severity(&c.severity),
            c.statement
        );

        match c.status {
            ConstraintStatus::Active => {
                println!(
                    "  ● Created     {}  {}",
                    c.timestamp.format("%Y-%m-%d"),
                    c.author.email
                );
                let age = render::format_age(&c.timestamp);
                // Strip " ago" suffix to show as duration
                let duration = age.trim_end_matches(" ago");
                println!("    (active {})", duration);
            }
            ConstraintStatus::Superseded => {
                println!(
                    "  ● Created     {}  {}",
                    c.timestamp.format("%Y-%m-%d"),
                    c.author.email
                );
                if let Some(ref sup_id) = c.superseded_by {
                    if let Some(sup_c) = constraint_map.get(sup_id.hex()) {
                        println!(
                            "  ▲ Superseded  {}  {}",
                            sup_c.timestamp.format("%Y-%m-%d"),
                            sup_c.author.email
                        );
                    } else {
                        println!("  ▲ Superseded");
                    }
                    println!("    └─ by {}", render::short_id(sup_id));
                } else {
                    println!("  ▲ Superseded");
                }
            }
            ConstraintStatus::Deprecated => {
                println!(
                    "  ✕ Deprecated  {}  {}",
                    c.timestamp.format("%Y-%m-%d"),
                    c.author.email
                );
                if let Some(ref reason) = c.deprecation_reason {
                    println!("    └─ reason: \"{}\"", reason);
                }
            }
        }

        println!();
    }

    // Summary
    let active = constraints
        .iter()
        .filter(|(_, c)| c.status == ConstraintStatus::Active)
        .count();
    let superseded = constraints
        .iter()
        .filter(|(_, c)| c.status == ConstraintStatus::Superseded)
        .count();
    let deprecated = constraints
        .iter()
        .filter(|(_, c)| c.status == ConstraintStatus::Deprecated)
        .count();

    println!(
        "Active: {}  Superseded: {}  Deprecated: {}",
        active, superseded, deprecated
    );

    Ok(())
}

fn print_json(constraints: &[(ObjectId, Constraint)]) -> Result<()> {
    let entries: Vec<_> = constraints
        .iter()
        .map(|(id, c)| {
            let mut entry = serde_json::json!({
                "id": id.hex(),
                "statement": c.statement,
                "severity": format!("{:?}", c.severity),
                "status": format!("{:?}", c.status),
                "timestamp": c.timestamp.to_rfc3339(),
                "author": format!("{} <{}>", c.author.name, c.author.email),
            });
            if let Some(ref sup_id) = c.superseded_by {
                entry["superseded_by"] = serde_json::json!(sup_id.hex());
            }
            if let Some(ref reason) = c.deprecation_reason {
                entry["deprecation_reason"] = serde_json::json!(reason);
            }
            entry
        })
        .collect();

    let active = constraints
        .iter()
        .filter(|(_, c)| c.status == ConstraintStatus::Active)
        .count();
    let superseded = constraints
        .iter()
        .filter(|(_, c)| c.status == ConstraintStatus::Superseded)
        .count();
    let deprecated = constraints
        .iter()
        .filter(|(_, c)| c.status == ConstraintStatus::Deprecated)
        .count();

    let output = serde_json::json!({
        "constraints": entries,
        "summary": {
            "active": active,
            "superseded": superseded,
            "deprecated": deprecated,
        }
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
