use chrono::{DateTime, Utc};
use owo_colors::OwoColorize;
use telos_core::hash::ObjectId;
use telos_core::object::constraint::{ConstraintSeverity, ConstraintStatus};

/// Color a constraint status string.
#[allow(dead_code)]
pub fn colored_status(status: &ConstraintStatus) -> String {
    match status {
        ConstraintStatus::Active => format!("{}", "Active".green()),
        ConstraintStatus::Superseded => format!("{}", "Superseded".yellow()),
        ConstraintStatus::Deprecated => format!("{}", "Deprecated".red()),
    }
}

/// Color a severity tag like [Must], [Should], [Prefer].
/// Padded to 8 visible characters for alignment.
pub fn colored_severity(severity: &ConstraintSeverity) -> String {
    match severity {
        ConstraintSeverity::Must => format!("[{}]  ", "Must".red()),
        ConstraintSeverity::Should => format!("[{}]", "Should".yellow()),
        ConstraintSeverity::Prefer => format!("[{}]", "Prefer".dimmed()),
    }
}

/// Return first 7 hex chars of an ObjectId (like git short hash).
pub fn short_id(id: &ObjectId) -> String {
    id.hex()[..7].to_string()
}

/// Return tree-drawing prefix for a node at the given nesting path.
/// `levels[i]` indicates whether the ancestor at depth i is the last child.
pub fn tree_prefix(levels: &[bool]) -> String {
    if levels.is_empty() {
        return String::new();
    }
    let mut prefix = String::new();
    for &is_last in &levels[..levels.len() - 1] {
        if is_last {
            prefix.push_str("   ");
        } else {
            prefix.push_str("│  ");
        }
    }
    if *levels.last().unwrap() {
        prefix.push_str("└─ ");
    } else {
        prefix.push_str("├─ ");
    }
    prefix
}

/// Return continuation prefix (no branch character) for content under a tree node.
pub fn tree_continuation(levels: &[bool]) -> String {
    let mut prefix = String::new();
    for &is_last in levels {
        if is_last {
            prefix.push_str("   ");
        } else {
            prefix.push_str("│  ");
        }
    }
    prefix
}

/// Render a proportional bar like `████░░░░`.
pub fn bar(count: usize, max: usize, width: usize) -> String {
    if max == 0 {
        return "░".repeat(width);
    }
    let filled = ((count * width + max - 1) / max).min(width);
    let empty = width - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

/// Truncate a string with ellipsis if it exceeds `width`.
#[allow(dead_code)]
pub fn truncate(s: &str, width: usize) -> String {
    if s.len() <= width {
        s.to_string()
    } else if width <= 3 {
        s[..width].to_string()
    } else {
        format!("{}...", &s[..width - 3])
    }
}

/// Format a timestamp as relative age ("3d ago", "2w ago", etc.).
pub fn format_age(timestamp: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(*timestamp);
    let mins = duration.num_minutes();
    let hours = duration.num_hours();
    let days = duration.num_days();

    if mins < 1 {
        "just now".to_string()
    } else if mins < 60 {
        format!("{}m ago", mins)
    } else if hours < 24 {
        format!("{}h ago", hours)
    } else if days < 7 {
        format!("{}d ago", days)
    } else if days < 30 {
        format!("{}w ago", days / 7)
    } else {
        format!("{}mo ago", days / 30)
    }
}

/// Return a bullet character for constraint status.
pub fn status_bullet(status: &ConstraintStatus) -> &'static str {
    match status {
        ConstraintStatus::Active => "●",
        _ => "○",
    }
}
