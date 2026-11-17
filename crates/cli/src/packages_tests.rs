// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

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
    assert_eq!(found, pairs(&[("crates/a", "alpha"), ("crates/b", "beta"), ("tool", "tool-cli")]));
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
fn go_packages_need_more_than_one_directory() {
    let single = detected(&[("go.mod", ""), ("main.go", "")], Language::Go);
    assert!(single.is_empty());
    let found = detected(
        &[("go.mod", ""), ("main.go", ""), ("pkg/util/u.go", ""), ("vendor/v/v.go", "")],
        Language::Go,
    );
    assert_eq!(found.len(), 2);
    assert!(found.contains(&("pkg/util".to_string(), "util".to_string())));
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
        &[("setup.py", "setup(name='flat')"), ("tests/__init__.py", ""), ("flat/__init__.py", "")],
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
