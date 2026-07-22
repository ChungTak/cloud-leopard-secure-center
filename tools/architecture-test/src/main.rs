use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Metadata {
    packages: Vec<Package>,
    workspace_members: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Package {
    name: String,
    manifest_path: String,
    dependencies: Vec<Dependency>,
    targets: Vec<Target>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Dependency {
    name: String,
    source: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Target {
    src_path: String,
    kind: Vec<String>,
}

fn layer(name: &str) -> Option<u8> {
    match name {
        "foundation" | "observability" => Some(0),
        n if n.starts_with("domain-") => Some(1),
        n if n == "http-api" => Some(4),
        n if n.ends_with("-api") => Some(2),
        "storage-postgres" | "message-local" => Some(4),
        "application" => Some(3),
        n if n.starts_with("security-") || n.starts_with("migration-") => Some(5),
        _ => None,
    }
}

fn is_external(dep: &Dependency) -> bool {
    dep.source.as_ref().is_some_and(|s| s.starts_with("registry+") || s.starts_with("git+"))
}

fn is_path(dep: &Dependency) -> bool {
    dep.source.is_none()
}

fn forbidden_for_domain(dep: &str) -> bool {
    matches!(
        dep,
        "tokio" | "axum" | "tonic" | "sqlx" | "async-nats" | "nats" | "reqwest" | "hyper" | "prost" | "actix-web"
    )
}

fn check<P: AsRef<Path>>(manifest_dir: P) -> Result<()> {
    let manifest_dir = manifest_dir.as_ref();
    let manifest = if manifest_dir.is_dir() {
        manifest_dir.join("Cargo.toml")
    } else {
        manifest_dir.to_path_buf()
    };

    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--manifest-path"])
        .arg(&manifest)
        .arg("--no-deps")
        .output()
        .context("failed to run cargo metadata")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("cargo metadata failed: {}", stderr);
    }

    let meta: Metadata = serde_json::from_slice(&output.stdout)
        .context("failed to parse cargo metadata")?;

    let pkg_by_id: HashMap<&str, &Package> = meta
        .packages
        .iter()
        .map(|p| (p.name.as_str(), p))
        .collect();

    let member_names: HashSet<&str> = meta
        .workspace_members
        .iter()
        .filter_map(|id| id.rsplit_once('#').map(|(path, _)| {
            Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or(path)
        }))
        .collect();

    for member_name in &member_names {
        let pkg = pkg_by_id.get(member_name).with_context(|| format!("workspace member {} not found in packages", member_name))?;
        let Some(l) = layer(&pkg.name) else {
            bail!("unknown layer for crate {}", pkg.name);
        };

        for dep in &pkg.dependencies {
            if is_external(dep) {
                if l == 1 && forbidden_for_domain(&dep.name) {
                    bail!(
                        "{}: domain crate must not depend on external framework {}",
                        pkg.name,
                        dep.name
                    );
                }
            } else if is_path(dep) {
                let dep_layer = layer(&dep.name);
                if let Some(dl) = dep_layer {
                    if dl > l {
                        bail!(
                            "{} (layer {}) depends on {} (layer {}), which violates architecture direction",
                            pkg.name,
                            l,
                            dep.name,
                            dl
                        );
                    }
                }
            }
        }

        check_source_for_crate(pkg, l)?;
    }

    Ok(())
}

fn check_source_for_crate(pkg: &Package, layer: u8) -> Result<()> {
    let manifest = Path::new(&pkg.manifest_path);
    let src_dir = manifest.parent().unwrap().join("src");
    if !src_dir.is_dir() {
        return Ok(());
    }

    let mut files = vec![];
    collect_rs_files(&src_dir, &mut files)?;

    for path in files {
        let text = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let lower = text.to_lowercase();

        if layer == 1 {
            for term in ["tokio::", "axum", "tonic", "sqlx", "nats::", "reqwest", "hyper::", "prost::", "actix"] {
                if lower.contains(term) {
                    bail!("{}: domain source contains forbidden term '{}'", path.display(), term);
                }
            }
        }

        if layer == 3 {
            for term in ["http_api", "storage_postgres", "message_local", "tokio::", "axum", "sqlx", "nats::", "reqwest", "hyper::"] {
                if lower.contains(term) {
                    bail!("{}: application source contains forbidden adapter/framework term '{}'", path.display(), term);
                }
            }
        }

        if layer == 5 {
            for term in ["sqlx", "select ", "insert ", "update ", "delete ", "domain_"] {
                if lower.contains(term) {
                    bail!("{}: app source contains forbidden business/SQL term '{}'", path.display(), term);
                }
            }
        }
    }

    Ok(())
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).context("read_dir")? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let path = std::env::args().nth(1).unwrap_or_else(|| ".".to_string());
    check(&path)?;
    println!("architecture-test passed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_file(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = fs::File::create(path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    fn make_workspace(tmp: &Path) {
        write_file(
            &tmp.join("Cargo.toml"),
            r#"[workspace]
resolver = "3"
members = ["crates/domain-bad", "crates/storage-bad-api"]
"#,
        );
        write_file(
            &tmp.join("crates/storage-bad-api/Cargo.toml"),
            r#"[package]
name = "storage-bad-api"
version = "0.1.0"
edition = "2024"
"#,
        );
        write_file(
            &tmp.join("crates/storage-bad-api/src/lib.rs"),
            "pub fn x() {}",
        );
    }

    #[test]
    fn real_workspace_passes() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        check(workspace).unwrap();
    }

    #[test]
    fn domain_to_port_layer_fails() {
        let tmp = tempfile::tempdir().unwrap();
        make_workspace(tmp.path());
        write_file(
            &tmp.path().join("crates/domain-bad/Cargo.toml"),
            r#"[package]
name = "domain-bad"
version = "0.1.0"
edition = "2024"

[dependencies]
storage-bad-api = { path = "../storage-bad-api" }
"#,
        );
        write_file(
            &tmp.path().join("crates/domain-bad/src/lib.rs"),
            "pub fn x() {}",
        );
        let err = check(tmp.path()).unwrap_err().to_string();
        assert!(err.contains("domain-bad"), "{}", err);
        assert!(err.contains("storage-bad-api"), "{}", err);
    }

    #[test]
    fn domain_source_framework_fails() {
        let tmp = tempfile::tempdir().unwrap();
        make_workspace(tmp.path());
        write_file(
            &tmp.path().join("crates/domain-bad/Cargo.toml"),
            r#"[package]
name = "domain-bad"
version = "0.1.0"
edition = "2024"
"#,
        );
        write_file(
            &tmp.path().join("crates/domain-bad/src/lib.rs"),
            "pub fn bad() { let _ = tokio::spawn(async {}); }",
        );
        let err = check(tmp.path()).unwrap_err().to_string();
        assert!(err.contains("tokio::"), "{}", err);
    }

    #[test]
    fn app_source_sql_fails() {
        let tmp = tempfile::tempdir().unwrap();
        write_file(
            &tmp.path().join("Cargo.toml"),
            r#"[workspace]
resolver = "3"
members = ["apps/security-bad"]
"#,
        );
        write_file(
            &tmp.path().join("apps/security-bad/Cargo.toml"),
            r#"[package]
name = "security-bad"
version = "0.1.0"
edition = "2024"
"#,
        );
        write_file(
            &tmp.path().join("apps/security-bad/src/main.rs"),
            r#"fn main() { let _ = "SELECT * FROM users"; }"#,
        );
        let err = check(tmp.path()).unwrap_err().to_string();
        assert!(err.contains("select "), "{}", err);
    }
}
