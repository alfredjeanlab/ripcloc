// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! Workspace package detection, for the per-package breakdown.

use std::fs;
use std::path::Path;

use crate::lang::Language;

/// A package of a workspace, at `path` relative to the project root.
#[derive(Debug, PartialEq, Eq)]
pub struct Package {
    pub path: String,
    pub name: String,
}

impl Package {
    fn new(path: impl Into<String>, name: impl Into<String>) -> Self {
        Self { path: path.into(), name: name.into() }
    }
}

/// Detect the packages of the project at `root`, sorted by path.
pub fn detect(root: &Path, language: Language) -> Vec<Package> {
    let mut packages = match language {
        Language::Rust => cargo_packages(root),
        Language::Go => go_packages(root),
        Language::JavaScript => js_packages(root),
        Language::Python => python_package(root).into_iter().collect(),
        Language::Ruby | Language::Shell | Language::Generic => vec![],
    };
    packages.sort_by(|a, b| a.path.cmp(&b.path));
    packages
}

/// The name in the manifest of the project at `dir`, such as a crate's `[package] name`.
pub fn manifest_name(dir: &Path, language: Language) -> Option<String> {
    match language {
        Language::Rust => read_toml(&dir.join("Cargo.toml"))
            .and_then(|t| Some(t.get("package")?.get("name")?.as_str()?.to_string())),
        Language::JavaScript => {
            let json: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(dir.join("package.json")).ok()?).ok()?;
            Some(json.get("name")?.as_str()?.to_string())
        }
        Language::Python => python_name(dir),
        Language::Go => {
            let module = fs::read_to_string(dir.join("go.mod")).ok()?;
            let path = module.lines().find_map(|l| l.trim().strip_prefix("module "))?.trim();
            Some(path.rsplit('/').next()?.to_string())
        }
        Language::Ruby => fs::read_dir(dir)
            .ok()?
            .flatten()
            .find_map(|e| Some(e.file_name().to_str()?.strip_suffix(".gemspec")?.to_string())),
        Language::Shell | Language::Generic => None,
    }
}

/// Expand workspace member patterns into directories relative to `root`.
///
/// A pattern is either a directory, or a directory followed by `/*` or `/**`, which both
/// select its immediate subdirectories. Other globs are not supported and are skipped.
fn expand_members(root: &Path, patterns: &[String]) -> Vec<String> {
    let mut dirs = Vec::new();
    for pattern in patterns {
        if let Some(base) = pattern.strip_suffix("/**").or_else(|| pattern.strip_suffix("/*")) {
            for entry in fs::read_dir(root.join(base)).into_iter().flatten().flatten() {
                if entry.path().is_dir() {
                    dirs.push(format!("{base}/{}", entry.file_name().to_string_lossy()));
                }
            }
        } else if !pattern.contains('*') {
            dirs.push(pattern.clone());
        }
    }
    dirs
}

fn read_toml(path: &Path) -> Option<toml::Table> {
    fs::read_to_string(path).ok()?.parse().ok()
}

fn strings<'a>(values: impl IntoIterator<Item = &'a toml::Value>) -> Vec<String> {
    values.into_iter().filter_map(|v| v.as_str().map(String::from)).collect()
}

fn cargo_packages(root: &Path) -> Vec<Package> {
    let Some(members) = read_toml(&root.join("Cargo.toml"))
        .and_then(|t| Some(strings(t.get("workspace")?.get("members")?.as_array()?)))
    else {
        return vec![];
    };
    expand_members(root, &members)
        .into_iter()
        .map(|path| {
            let name = read_toml(&root.join(&path).join("Cargo.toml"))
                .and_then(|t| Some(t.get("package")?.get("name")?.as_str()?.to_string()))
                .unwrap_or_else(|| path.clone());
            Package::new(path, name)
        })
        .collect()
}

/// Directories with `.go` files, when there is more than one, named by their last component.
fn go_packages(root: &Path) -> Vec<Package> {
    fn walk(root: &Path, dir: &Path, packages: &mut Vec<Package>) {
        let mut has_go = false;
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if entry.file_name() == "vendor" {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, packages);
            } else if path.extension().is_some_and(|e| e == "go") {
                has_go = true;
            }
        }
        if has_go {
            let rel = dir.strip_prefix(root).unwrap_or(dir).to_string_lossy().into_owned();
            let rel = if rel.is_empty() { ".".to_string() } else { rel };
            let name = rel.rsplit('/').next().unwrap_or(&rel).to_string();
            packages.push(Package::new(rel, name));
        }
    }
    let mut packages = Vec::new();
    walk(root, root, &mut packages);
    if packages.len() > 1 { packages } else { vec![] }
}

/// Members of a pnpm or npm/yarn workspace that have a `package.json`, named by directory.
fn js_packages(root: &Path) -> Vec<Package> {
    let pnpm = || -> Option<Vec<String>> {
        let yaml: serde_yaml::Value =
            serde_yaml::from_str(&fs::read_to_string(root.join("pnpm-workspace.yaml")).ok()?)
                .ok()?;
        let seq = yaml.get("packages")?.as_sequence()?;
        Some(seq.iter().filter_map(|v| v.as_str().map(String::from)).collect())
    };
    let npm = || -> Option<Vec<String>> {
        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("package.json")).ok()?).ok()?;
        let workspaces = json.get("workspaces")?;
        let list = workspaces.as_array().or_else(|| workspaces.get("packages")?.as_array())?;
        Some(list.iter().filter_map(|v| v.as_str().map(String::from)).collect())
    };
    let patterns = pnpm().filter(|p| !p.is_empty()).or_else(npm).unwrap_or_default();
    expand_members(root, &patterns)
        .into_iter()
        .filter(|path| root.join(path).join("package.json").exists())
        .map(|path| {
            let name = path.rsplit('/').next().unwrap_or(&path).to_string();
            Package::new(path, name)
        })
        .collect()
}

/// The name in `pyproject.toml`, or else in `setup.py`.
fn python_name(root: &Path) -> Option<String> {
    read_toml(&root.join("pyproject.toml"))
        .and_then(|t| Some(t.get("project")?.get("name")?.as_str()?.to_string()))
        .or_else(|| {
            let setup = fs::read_to_string(root.join("setup.py")).ok()?;
            let re = regex::Regex::new(r#"name\s*=\s*["']([^"']+)["']"#).ok()?;
            Some(re.captures(&setup)?.get(1)?.as_str().to_string())
        })
}

/// The package directory of a Python project, named from `pyproject.toml` or `setup.py`.
///
/// It is the first of a package under `src/`, `src/` itself, or a package in the root. A project
/// with a name but without any of those is a package at the root.
fn python_package(root: &Path) -> Option<Package> {
    let name = python_name(root);
    let subdirs = |dir: &Path| -> Vec<(String, std::path::PathBuf)> {
        let mut dirs: Vec<_> = fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
            .filter(|(n, _)| !n.starts_with('_') && !n.starts_with('.'))
            .collect();
        dirs.sort();
        dirs
    };
    let package = |path: String, dir: &str| {
        Package::new(path, name.clone().unwrap_or_else(|| dir.replace('-', "_")))
    };

    let src = root.join("src");
    if let Some((dir, _)) = subdirs(&src)
        .into_iter()
        .find(|(_, p)| p.join("__init__.py").exists() || p.join("py.typed").exists())
    {
        return Some(package(format!("src/{dir}"), &dir));
    }
    let src_has_py = fs::read_dir(&src)
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| e.path().extension().is_some_and(|x| x == "py"));
    if src_has_py && let Some(name) = &name {
        return Some(Package::new("src", name.clone()));
    }
    if let Some((dir, _)) = subdirs(root).into_iter().find(|(n, p)| {
        !["tests", "test", "docs", "build", "dist"].contains(&n.as_str())
            && p.join("__init__.py").exists()
    }) {
        return Some(package(dir.clone(), &dir));
    }
    name.map(|name| Package::new(".", name))
}

#[cfg(test)]
#[path = "packages_tests.rs"]
mod tests;
