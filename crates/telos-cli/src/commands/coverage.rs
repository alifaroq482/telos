use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::env;
use telos_store::repository::Repository;

use crate::render;

#[derive(Clone)]
struct FileEntry {
    name: String,
    count: usize,
}

struct DirNode {
    #[allow(dead_code)]
    name: String,
    files: Vec<FileEntry>,
    subdirs: BTreeMap<String, DirNode>,
}

impl DirNode {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            files: Vec::new(),
            subdirs: BTreeMap::new(),
        }
    }

    fn insert(&mut self, parts: &[&str], count: usize) {
        if parts.len() == 1 {
            self.files.push(FileEntry {
                name: parts[0].to_string(),
                count,
            });
            return;
        }
        let dir_name = parts[0];
        let subdir = self
            .subdirs
            .entry(dir_name.to_string())
            .or_insert_with(|| DirNode::new(dir_name));
        subdir.insert(&parts[1..], count);
    }
}

/// Source file extensions to include in coverage analysis.
const SOURCE_EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "rb", "c", "cpp", "h", "hpp", "cs",
    "swift", "kt", "scala", "vue", "svelte", "ex", "exs",
];

pub fn run(dir: Option<String>, json: bool) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd).context("not a Telos repository")?;

    // Get tracked source files
    let mut args = vec!["ls-files"];
    let dir_ref;
    if let Some(ref d) = dir {
        args.push("--");
        dir_ref = d.clone();
        args.push(&dir_ref);
    }
    let output = std::process::Command::new("git")
        .args(&args)
        .current_dir(repo.root())
        .output()
        .context("failed to run git ls-files")?;

    let files: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .filter(|l| {
            l.rsplit('.')
                .next()
                .map(|ext| SOURCE_EXTENSIONS.contains(&ext))
                .unwrap_or(false)
        })
        .map(String::from)
        .collect();

    if files.is_empty() {
        println!("No source files found.");
        return Ok(());
    }

    // Count bindings per file
    let mut file_counts: Vec<(String, usize)> = Vec::new();
    let mut total_constraints = 0;
    let mut covered_files = 0;

    for file in &files {
        let count = repo.indexes.by_path(file).len();
        if count > 0 {
            covered_files += 1;
            total_constraints += count;
        }
        file_counts.push((file.clone(), count));
    }

    let total_files = files.len();
    let max_count = file_counts.iter().map(|(_, c)| *c).max().unwrap_or(0);

    if json {
        return print_json(&file_counts, total_files, covered_files, total_constraints);
    }

    // Build directory tree
    let mut root = DirNode::new("");
    for (path, count) in &file_counts {
        let parts: Vec<&str> = path.split('/').collect();
        root.insert(&parts, *count);
    }

    // Find max filename length for alignment
    let max_name_len = file_counts
        .iter()
        .map(|(p, _)| p.split('/').last().unwrap_or(p).len())
        .max()
        .unwrap_or(12);

    println!("Constraint Coverage");
    println!("───────────────────");
    println!();

    render_tree(&root, max_name_len, max_count);

    // Summary
    println!();
    let pct = if total_files > 0 {
        covered_files * 100 / total_files
    } else {
        0
    };
    println!(
        "Summary: {} constraint{} across {}/{} files ({}%)",
        total_constraints,
        if total_constraints == 1 { "" } else { "s" },
        covered_files,
        total_files,
        pct
    );
    if total_files > covered_files {
        println!(
            "         {} file{} uncovered",
            total_files - covered_files,
            if total_files - covered_files == 1 {
                ""
            } else {
                "s"
            }
        );
    }

    Ok(())
}

fn render_tree(root: &DirNode, max_name_len: usize, max_count: usize) {
    // Top-level entries without tree connectors
    let dir_names: Vec<_> = root.subdirs.keys().collect();
    for (i, name) in dir_names.iter().enumerate() {
        let subdir = &root.subdirs[*name];
        let is_last = i == dir_names.len() - 1 && root.files.is_empty();
        let prefix = if is_last { "   " } else { "│  " };
        println!("{}/", name);
        render_subtree(subdir, prefix, max_name_len, max_count);
    }
    // Top-level files (rare but possible)
    for file in &root.files {
        let bar_str = render::bar(file.count, max_count, 8);
        let count_label = format_count(file.count);
        println!(
            "{:<width$} {} {}",
            file.name,
            bar_str,
            count_label,
            width = max_name_len
        );
    }
}

fn render_subtree(node: &DirNode, prefix: &str, max_name_len: usize, max_count: usize) {
    let subdir_names: Vec<_> = node.subdirs.keys().collect();
    let total_children = subdir_names.len() + node.files.len();
    let mut idx = 0;

    for name in &subdir_names {
        idx += 1;
        let is_last = idx == total_children;
        let connector = if is_last { "└─ " } else { "├─ " };
        let child_prefix = if is_last {
            format!("{}   ", prefix)
        } else {
            format!("{}│  ", prefix)
        };
        println!("{}{}{}/", prefix, connector, name);
        render_subtree(&node.subdirs[*name], &child_prefix, max_name_len, max_count);
    }

    // Sort files: covered files first (by count desc), then uncovered
    let mut sorted_files = node.files.clone();
    sorted_files.sort_by(|a, b| b.count.cmp(&a.count));

    for file in &sorted_files {
        idx += 1;
        let is_last = idx == total_children;
        let connector = if is_last { "└─ " } else { "├─ " };
        let bar_str = render::bar(file.count, max_count, 8);
        let count_label = format_count(file.count);
        println!(
            "{}{}{:<width$} {} {}",
            prefix,
            connector,
            file.name,
            bar_str,
            count_label,
            width = max_name_len
        );
    }
}

fn format_count(count: usize) -> String {
    if count == 0 {
        "-".to_string()
    } else {
        format!(
            "{} constraint{}",
            count,
            if count == 1 { "" } else { "s" }
        )
    }
}

fn print_json(
    file_counts: &[(String, usize)],
    total_files: usize,
    covered_files: usize,
    total_constraints: usize,
) -> Result<()> {
    let files: Vec<_> = file_counts
        .iter()
        .map(|(path, count)| {
            serde_json::json!({
                "path": path,
                "constraint_count": count,
            })
        })
        .collect();

    let output = serde_json::json!({
        "files": files,
        "summary": {
            "total_files": total_files,
            "covered_files": covered_files,
            "uncovered_files": total_files - covered_files,
            "total_constraints": total_constraints,
            "coverage_percent": if total_files > 0 { covered_files * 100 / total_files } else { 0 },
        }
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
