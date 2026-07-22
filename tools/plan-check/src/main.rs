use anyhow::{Context, Result, bail};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct Task {
    id: String,
    file: PathBuf,
    title: String,
    prerequisites: Vec<String>,
    checkboxes: Vec<(bool, String)>,
}

impl Task {
    fn completed(&self) -> bool {
        !self.checkboxes.is_empty() && self.checkboxes.iter().all(|(checked, _)| *checked)
    }
}

fn main() -> Result<()> {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "dev-docs/001_vibe_coding_plan".to_string());
    let plan_dir = PathBuf::from(dir);
    run(&plan_dir)
}

fn run(plan_dir: &Path) -> Result<()> {
    let mut tasks: Vec<Task> = Vec::new();
    let mut seen_ids: HashMap<String, PathBuf> = HashMap::new();

    let task_header = Regex::new(r"^###\s+([A-Z]+(?:-[A-Z]+)*-\d+)\s*[:：]?(.*)$").unwrap();
    let prereq_line = Regex::new(r"\*\*前置[:：]\*\*\s*(.*)$").unwrap();
    let checkbox = Regex::new(r"^-\s+\[([xX ])\]\s+(.*)$").unwrap();
    let id_pattern = Regex::new(r"[A-Z]+(?:-[A-Z]+)*-\d+").unwrap();

    let md_files: Vec<PathBuf> = walkdir::WalkDir::new(plan_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
        .map(|e| e.path().to_path_buf())
        .collect();

    for path in &md_files {
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let mut current: Option<Task> = None;
        for line in text.lines() {
            if let Some(cap) = task_header.captures(line) {
                if let Some(t) = current.take() {
                    tasks.push(t);
                }
                let id = cap[1].to_string();
                let title = cap[2].trim().to_string();
                if let Some(prev) = seen_ids.insert(id.clone(), path.clone()) {
                    bail!("duplicate task ID {} in {} and {}", id, prev.display(), path.display());
                }
                current = Some(Task {
                    id,
                    file: path.clone(),
                    title,
                    prerequisites: Vec::new(),
                    checkboxes: Vec::new(),
                });
            } else if let Some(t) = current.as_mut() {
                if let Some(cap) = prereq_line.captures(line) {
                    let raw = cap[1].to_string();
                    for m in id_pattern.find_iter(&raw) {
                        t.prerequisites.push(m.as_str().to_string());
                    }
                } else if let Some(cap) = checkbox.captures(line) {
                    let checked = cap[1].trim().to_ascii_lowercase() == "x";
                    let text = cap[2].trim().to_string();
                    t.checkboxes.push((checked, text));
                }
            }
        }
        if let Some(t) = current {
            tasks.push(t);
        }
    }

    // Unique IDs already enforced during parse.
    let id_set: HashSet<&str> = tasks.iter().map(|t| t.id.as_str()).collect();

    // Prerequisite existence.
    for t in &tasks {
        for p in &t.prerequisites {
            if !id_set.contains(p.as_str()) {
                bail!(
                    "{}: prerequisite {} not found",
                    t.id,
                    p
                );
            }
        }
    }

    // Cycle detection.
    let mut visited = HashSet::new();
    let mut stack = HashSet::new();
    for t in &tasks {
        if !visited.contains(&t.id) {
            dfs(t.id.as_str(), &tasks, &mut visited, &mut stack)?;
        }
    }

    // Completed tasks must have a report.
    for t in &tasks {
        if t.completed() {
            let report_name = t.id.to_ascii_lowercase();
            let report_path = plan_dir.join("reports").join(format!("{}.md", report_name));
            if !report_path.exists() {
                bail!(
                    "{} is marked completed but report {} is missing",
                    t.id,
                    report_path.display()
                );
            }
        }
    }

    // Relative links.
    for path in &md_files {
        let text = fs::read_to_string(path)?;
        check_links(path, &text, plan_dir)?;
    }

    // Code fences balanced.
    for path in &md_files {
        let text = fs::read_to_string(path)?;
        check_code_fences(path, &text)?;
    }

    // Placeholder expressions (skip inline/triple-backtick code).
    let placeholder = Regex::new(r"(?i)(todo!\(\)|unimplemented!\(\)|unimplemented\(\)|fixme|xxx\b|hack\b|stub\b)").unwrap();
    for path in &md_files {
        let text = fs::read_to_string(path)?;
        let prose = strip_code(&text);
        for m in placeholder.find_iter(&prose) {
            bail!("{}: placeholder expression '{}' in prose", path.display(), m.as_str());
        }
    }

    println!("plan-check passed: {} tasks, {} files", tasks.len(), md_files.len());
    Ok(())
}

fn dfs(
    id: &str,
    tasks: &[Task],
    visited: &mut HashSet<String>,
    stack: &mut HashSet<String>,
) -> Result<()> {
    if stack.contains(id) {
        bail!("prerequisite cycle involving {}", id);
    }
    if visited.contains(id) {
        return Ok(());
    }
    stack.insert(id.to_string());
    let task = tasks
        .iter()
        .find(|t| t.id == id)
        .expect("task must exist");
    for p in &task.prerequisites {
        dfs(p, tasks, visited, stack)?;
    }
    stack.remove(id);
    visited.insert(id.to_string());
    Ok(())
}

fn check_links(path: &Path, text: &str, plan_dir: &Path) -> Result<()> {
    let link_re = Regex::new(r"!?\[([^\]]*)\]\(([^\)]+)\)").unwrap();
    let bad_schemes = ["http:", "https:", "ftp:", "mailto:"];
    for cap in link_re.captures_iter(text) {
        let url = cap[2].trim();
        if url.is_empty() || url.starts_with('#') || bad_schemes.iter().any(|s| url.starts_with(s)) {
            continue;
        }
        let resolved = if url.starts_with('/') {
            plan_dir.parent().unwrap().join(url.strip_prefix('/').unwrap())
        } else {
            path.parent().unwrap().join(url)
        };
        // Allow mailto and empty anchors already skipped.
        if !resolved.exists() {
            bail!("{}: broken relative link '{}' -> {}", path.display(), url, resolved.display());
        }
    }
    Ok(())
}

fn check_code_fences(path: &Path, text: &str) -> Result<()> {
    let fence_re = Regex::new(r"^```").unwrap();
    let mut open = false;
    for line in text.lines() {
        if fence_re.is_match(line) {
            open = !open;
        }
    }
    if open {
        bail!("{}: unclosed code fence", path.display());
    }
    Ok(())
}

fn strip_code(text: &str) -> String {
    // Remove triple-backtick code blocks and inline `code` spans.
    let triple = Regex::new(r"```[\s\S]*?```").unwrap();
    let inline = Regex::new(r"`[^`]*`").unwrap();
    let mut out = triple.replace_all(text, "\n").to_string();
    out = inline.replace_all(&out, " ").to_string();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_plan(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn pass_valid_plan() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("reports")).unwrap();
        let plan = r#"# Plan

### BAS-001: baseline

**前置：** 无。

- [x] first
- [x] second
"#;
        make_plan(tmp.path(), "01.md", plan);
        fs::File::create(tmp.path().join("reports").join("bas-001.md")).unwrap();
        run(tmp.path()).unwrap();
    }

    #[test]
    fn fail_missing_prerequisite() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("reports")).unwrap();
        let plan = r#"# Plan

### BAS-002: checker

**前置：** BAS-001。

- [x] item
"#;
        make_plan(tmp.path(), "01.md", plan);
        let err = run(tmp.path()).unwrap_err().to_string();
        assert!(err.contains("prerequisite BAS-001 not found"), "{}", err);
    }

    #[test]
    fn fail_cycle() {
        let tmp = tempfile::tempdir().unwrap();
        let plan = r#"# Plan

### BAS-001: a

**前置：** BAS-002。

- [ ] item

### BAS-002: b

**前置：** BAS-001。

- [ ] item
"#;
        make_plan(tmp.path(), "01.md", plan);
        let err = run(tmp.path()).unwrap_err().to_string();
        assert!(err.contains("cycle"), "{}", err);
    }

    #[test]
    fn fail_missing_report() {
        let tmp = tempfile::tempdir().unwrap();
        let plan = r#"# Plan

### BAS-001: baseline

**前置：** 无。

- [x] item
"#;
        make_plan(tmp.path(), "01.md", plan);
        let err = run(tmp.path()).unwrap_err().to_string();
        assert!(err.contains("report"), "{}", err);
    }

    #[test]
    fn fail_broken_link() {
        let tmp = tempfile::tempdir().unwrap();
        let plan = r#"# Plan

### BAS-001: baseline

**前置：** 无。

- [ ] item

[link](missing.md)
"#;
        make_plan(tmp.path(), "01.md", plan);
        let err = run(tmp.path()).unwrap_err().to_string();
        assert!(err.contains("broken relative link"), "{}", err);
    }

    #[test]
    fn fail_unclosed_fence() {
        let tmp = tempfile::tempdir().unwrap();
        let plan = r#"# Plan

### BAS-001: baseline

**前置：** 无。

- [ ] item

```rust
unclosed
"#;
        make_plan(tmp.path(), "01.md", plan);
        let err = run(tmp.path()).unwrap_err().to_string();
        assert!(err.contains("unclosed code fence"), "{}", err);
    }

    #[test]
    fn fail_placeholder_in_prose() {
        let tmp = tempfile::tempdir().unwrap();
        let plan = r#"# Plan

### BAS-001: baseline

**前置：** 无。

- [ ] item

This paragraph contains a todo!() expression.
"#;
        make_plan(tmp.path(), "01.md", plan);
        let err = run(tmp.path()).unwrap_err().to_string();
        assert!(err.contains("placeholder"), "{}", err);
    }
}
