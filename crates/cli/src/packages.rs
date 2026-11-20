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
        // A Go module's packages are found as its directories are resolved.
        Language::Go => vec![],
        Language::JavaScript => js_packages(root),
        Language::Python => python_package(root).into_iter().collect(),
        Language::Ruby
        | Language::Dart
        | Language::Swift
        | Language::Jvm
        | Language::Shell
        | Language::Generic => vec![],
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
        Language::Dart | Language::Swift | Language::Jvm | Language::Shell | Language::Generic => {
            None
        }
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

/// Workspace members, and the workspace root when it is a crate too.
fn cargo_packages(root: &Path) -> Vec<Package> {
    let package_name = |t: &toml::Table| Some(t.get("package")?.get("name")?.as_str()?.to_string());
    let Some(manifest) = read_toml(&root.join("Cargo.toml")) else { return vec![] };
    let Some(members) =
        manifest.get("workspace").and_then(|w| Some(strings(w.get("members")?.as_array()?)))
    else {
        return vec![];
    };
    let mut packages: Vec<Package> = expand_members(root, &members)
        .into_iter()
        .map(|path| {
            let name = read_toml(&root.join(&path).join("Cargo.toml"))
                .and_then(|t| package_name(&t))
                .unwrap_or_else(|| path.clone());
            Package::new(path, name)
        })
        .collect();
    if let Some(name) = package_name(&manifest) {
        packages.push(Package::new(".", name));
    }
    packages
}

/// Whether more than one directory below `root`, outside `vendor`, has `.go` files, in which case
/// each such directory is a package named by its last component.
///
/// The search stops at the second such directory, so it reads little of a large module.
pub fn go_has_packages(root: &Path) -> bool {
    fn count(dir: &Path, found: &mut usize) {
        let mut subdirs = Vec::new();
        let mut has_go = false;
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let name = entry.file_name();
            if name == "vendor" {
                continue;
            }
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                subdirs.push(entry.path());
            } else if Path::new(&name).extension().is_some_and(|e| e == "go") {
                has_go = true;
            }
        }
        *found += usize::from(has_go);
        for subdir in subdirs {
            if *found > 1 {
                return;
            }
            count(&subdir, found);
        }
    }
    let mut found = 0;
    count(root, &mut found);
    found > 1
}

/// Members of a pnpm or npm/yarn workspace that have a `package.json`, named by directory.
fn js_packages(root: &Path) -> Vec<Package> {
    let pnpm = || -> Option<Vec<String>> {
        Some(pnpm_packages(&fs::read_to_string(root.join("pnpm-workspace.yaml")).ok()?))
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

/// The `packages` list of a `pnpm-workspace.yaml`.
///
/// This is not a YAML parser. It reads the forms that pnpm documents, which are a block sequence
/// under a top-level `packages` key and a flow sequence such as `packages: ['a/*', 'b']`, with
/// quoted or plain items and `#` comments. Anything else yields an empty list.
fn pnpm_packages(yaml: &str) -> Vec<String> {
    let mut lines = yaml.lines().map(strip_comment);
    let Some(rest) = lines.by_ref().find_map(|l| l.strip_prefix("packages:")) else {
        return vec![];
    };
    let rest = rest.trim();
    if rest.starts_with('[') {
        // A flow sequence may continue over the following lines until its closing bracket.
        let mut flow = rest.to_string();
        while !flow.contains(']') {
            let Some(line) = lines.next() else { break };
            flow.push(' ');
            flow.push_str(line);
        }
        let inner = flow.trim_start_matches('[').split(']').next().unwrap_or_default();
        return split_unquoted(inner, ',')
            .into_iter()
            .map(unquote)
            .filter(|s| !s.is_empty())
            .collect();
    }
    if !rest.is_empty() {
        return vec![];
    }
    // The block ends at the next line that starts in the first column and is not an item, which is
    // the next top-level key.
    lines
        .filter(|l| !l.trim().is_empty())
        .take_while(|l| l.starts_with([' ', '\t', '-']))
        .filter_map(|l| l.trim_start().strip_prefix('-'))
        .map(unquote)
        .filter(|s| !s.is_empty())
        .collect()
}

/// The line without a YAML comment, which starts at a `#` that is outside quotes and either begins
/// the line or follows whitespace.
fn strip_comment(line: &str) -> &str {
    let mut quote = None;
    let mut previous = ' ';
    for (i, c) in line.char_indices() {
        match (quote, c) {
            (None, '#') if previous.is_whitespace() => return line.get(..i).unwrap_or(line),
            (None, '\'' | '"') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            _ => {}
        }
        previous = c;
    }
    line
}

/// The pieces of `text` between each `separator` that is outside quotes.
fn split_unquoted(text: &str, separator: char) -> Vec<&str> {
    let (mut pieces, mut start, mut quote) = (Vec::new(), 0, None);
    for (i, c) in text.char_indices() {
        match (quote, c) {
            (None, '\'' | '"') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            (None, _) if c == separator => {
                pieces.extend(text.get(start..i));
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    pieces.extend(text.get(start..));
    pieces
}

/// A YAML scalar without surrounding whitespace and quotes.
fn unquote(scalar: &str) -> String {
    let scalar = scalar.trim();
    ['\'', '"']
        .iter()
        .find_map(|&q| scalar.strip_prefix(q)?.strip_suffix(q))
        .unwrap_or(scalar)
        .to_string()
}

/// The name in `pyproject.toml`, or else in `setup.py`.
fn python_name(root: &Path) -> Option<String> {
    read_toml(&root.join("pyproject.toml"))
        .and_then(|t| Some(t.get("project")?.get("name")?.as_str()?.to_string()))
        .or_else(|| setup_py_name(&fs::read_to_string(root.join("setup.py")).ok()?))
}

/// The first quoted string assigned to something that ends in `name` in a `setup.py`, as in
/// `setup(name="pkg")`.
///
/// The opening and closing quotes do not have to match, and the string cannot be empty or contain
/// either kind of quote.
fn setup_py_name(setup: &str) -> Option<String> {
    setup.match_indices("name").find_map(|(at, word)| {
        let value = setup.get(at + word.len()..)?.trim_start().strip_prefix('=')?.trim_start();
        let value = value.strip_prefix(['"', '\''])?;
        let end = value.find(['"', '\''])?;
        value.get(..end).filter(|name| !name.is_empty()).map(String::from)
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
mod tests {
    use super::*;

    fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, content) in files {
            let path = dir.path().join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }
        dir
    }

    fn detected(files: &[(&str, &str)], language: Language) -> Vec<(String, String)> {
        let dir = tree(files);
        detect(dir.path(), language).into_iter().map(|p| (p.path, p.name)).collect()
    }

    fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
        expected.iter().map(|(p, n)| (p.to_string(), n.to_string())).collect()
    }

    #[test]
    fn cargo_workspace_members_are_named_from_their_manifests() {
        let found = detected(
            &[
                ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\", \"tool\"]\n"),
                ("crates/a/Cargo.toml", "[package]\nname = \"alpha\"\n"),
                ("crates/b/Cargo.toml", "[package]\nname = \"beta\"\n"),
                ("tool/Cargo.toml", "[package]\nname = \"tool-cli\"\n"),
            ],
            Language::Rust,
        );
        assert_eq!(
            found,
            pairs(&[("crates/a", "alpha"), ("crates/b", "beta"), ("tool", "tool-cli")])
        );
    }

    #[test]
    fn a_workspace_root_that_is_a_crate_is_a_package() {
        let found = detected(
            &[
                ("Cargo.toml", "[package]\nname = \"root\"\n\n[workspace]\nmembers = [\"cli\"]\n"),
                ("cli/Cargo.toml", "[package]\nname = \"root-cli\"\n"),
            ],
            Language::Rust,
        );
        assert_eq!(found, pairs(&[(".", "root"), ("cli", "root-cli")]));
    }

    #[test]
    fn single_crate_has_no_packages() {
        let found = detected(&[("Cargo.toml", "[package]\nname = \"x\"\n")], Language::Rust);
        assert!(found.is_empty());
    }

    #[test]
    fn npm_workspaces_need_a_package_json() {
        let found = detected(
            &[
                ("package.json", r#"{"workspaces": ["packages/*"]}"#),
                ("packages/core/package.json", "{}"),
                ("packages/notes/README.md", ""),
            ],
            Language::JavaScript,
        );
        assert_eq!(found, pairs(&[("packages/core", "core")]));
    }

    #[test]
    fn pnpm_workspace_takes_precedence() {
        let found = detected(
            &[
                ("pnpm-workspace.yaml", "packages:\n  - apps/*\n"),
                ("package.json", r#"{"workspaces": {"packages": ["libs/*"]}}"#),
                ("apps/web/package.json", "{}"),
                ("libs/ui/package.json", "{}"),
            ],
            Language::JavaScript,
        );
        assert_eq!(found, pairs(&[("apps/web", "web")]));
    }

    #[test]
    fn pnpm_block_sequence_with_quotes_comments_and_a_following_key() {
        let yaml = "\
# workspace
packages:
  # apps
  - apps/*
  - 'libs/**'  # shared
  - \"tools/#cli\"
  - '!**/test/**'

catalog:
  - not-a-package
";
        assert_eq!(pnpm_packages(yaml), ["apps/*", "libs/**", "tools/#cli", "!**/test/**"]);
    }

    #[test]
    fn pnpm_items_may_start_in_the_first_column() {
        assert_eq!(pnpm_packages("packages:\n- a\n- b\nother: 1\n"), ["a", "b"]);
    }

    #[test]
    fn pnpm_flow_sequence() {
        assert_eq!(pnpm_packages("packages: ['a/*', \"b,c\", d] # x\n"), ["a/*", "b,c", "d"]);
        assert_eq!(pnpm_packages("packages: [\n  a,\n  b\n]\nnext: [c]\n"), ["a", "b"]);
    }

    #[test]
    fn pnpm_without_a_top_level_packages_list() {
        assert!(pnpm_packages("catalog:\n  packages:\n    - a\n").is_empty());
        assert!(pnpm_packages("packages: apps\n").is_empty());
        assert!(pnpm_packages("").is_empty());
    }

    #[test]
    fn setup_py_names() {
        assert_eq!(setup_py_name("setup(\n    name = \"pkg\",\n)").as_deref(), Some("pkg"));
        assert_eq!(setup_py_name("setup(version='1', name='x')").as_deref(), Some("x"));
        assert_eq!(setup_py_name("name = ''\nname='second'").as_deref(), Some("second"));
        assert_eq!(setup_py_name("name = NAME"), None);
    }

    #[test]
    fn go_packages_need_more_than_one_directory() {
        let single = tree(&[("go.mod", ""), ("main.go", ""), ("vendor/v/v.go", "")]);
        assert!(!go_has_packages(single.path()));
        let many = tree(&[("go.mod", ""), ("main.go", ""), ("pkg/util/u.go", "")]);
        assert!(go_has_packages(many.path()));
    }

    #[test]
    fn python_src_layout_is_named_from_pyproject() {
        let found = detected(
            &[("pyproject.toml", "[project]\nname = \"my-lib\"\n"), ("src/my_lib/__init__.py", "")],
            Language::Python,
        );
        assert_eq!(found, pairs(&[("src/my_lib", "my-lib")]));
    }

    #[test]
    fn python_flat_layout_skips_tests_and_falls_back_to_setup_py() {
        let found = detected(
            &[
                ("setup.py", "setup(name='flat')"),
                ("tests/__init__.py", ""),
                ("flat/__init__.py", ""),
            ],
            Language::Python,
        );
        assert_eq!(found, pairs(&[("flat", "flat")]));
    }

    #[test]
    fn python_name_without_package_directory_is_the_root() {
        let found = detected(&[("pyproject.toml", "[project]\nname = \"x\"\n")], Language::Python);
        assert_eq!(found, pairs(&[(".", "x")]));
    }

    #[test]
    fn manifest_names() {
        let dir = tree(&[
            ("rs/Cargo.toml", "[package]\nname = \"crate-name\"\n"),
            ("js/package.json", r#"{"name": "@scope/web"}"#),
            ("go/go.mod", "module example.com/org/svc\n"),
            ("rb/thing.gemspec", ""),
        ]);
        let name = |sub: &str, language| manifest_name(&dir.path().join(sub), language);
        assert_eq!(name("rs", Language::Rust).as_deref(), Some("crate-name"));
        assert_eq!(name("js", Language::JavaScript).as_deref(), Some("@scope/web"));
        assert_eq!(name("go", Language::Go).as_deref(), Some("svc"));
        assert_eq!(name("rb", Language::Ruby).as_deref(), Some("thing"));
    }
}
