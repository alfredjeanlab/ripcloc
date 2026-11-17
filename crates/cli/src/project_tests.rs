// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

fn tree(files: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, content) in files.iter().map(|f| f.split_once('=').unwrap_or((f, ""))) {
        let path = dir.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content.replace("\\n", "\n")).unwrap();
    }
    dir
}

// A monorepo whose root has no manifest, with a Rust workspace, a lone crate and a JS app.
fn monorepo() -> tempfile::TempDir {
    tree(&[
        "engine/Cargo.toml=[workspace]\\nmembers = [\"crates/*\"]\\n",
        "engine/crates/core/Cargo.toml=[package]\\nname = \"engine-core\"\\n",
        "engine/crates/core/src/lib.rs",
        "engine/crates/core/python/requirements.txt",
        "tool/Cargo.toml=[package]\\nname = \"the-tool\"\\n",
        "tool/src/main.rs",
        "web/package.json={\"name\": \"web-app\"}",
        "web/src/index.ts",
        "scripts/run.sh",
    ])
}

#[test]
fn nested_manifests_start_projects_with_their_own_language() {
    let dir = monorepo();
    let resolver = Resolver::new([dir.path().to_path_buf()]);
    assert_eq!(resolver.context(dir.path()).language, Language::Shell);
    assert_eq!(
        resolver.context(&dir.path().join("engine/crates/core/src")).language,
        Language::Rust
    );
    assert_eq!(resolver.context(&dir.path().join("web/src")).language, Language::JavaScript);
    let python = resolver.context(&dir.path().join("engine/crates/core/python"));
    assert_eq!(python.language, Language::Python);
    assert_eq!(&*python.project, dir.path().join("engine/crates/core/python"));
    assert_eq!(&*python.root, dir.path());
}

#[test]
fn files_belong_to_their_innermost_package() {
    let dir = monorepo();
    let resolver = Resolver::new([dir.path().to_path_buf()]);
    let package =
        |sub: &str| resolver.context(&dir.path().join(sub)).package.as_deref().map(String::from);
    assert_eq!(package("engine/crates/core/src").as_deref(), Some("engine-core"));
    // A nested project without a named manifest stays in the enclosing package.
    assert_eq!(package("engine/crates/core/python").as_deref(), Some("engine-core"));
    assert_eq!(package("tool/src").as_deref(), Some("the-tool"));
    assert_eq!(package("web/src").as_deref(), Some("web-app"));
    assert_eq!(package("scripts"), None);
    assert_eq!(package("engine"), None);
}

#[test]
fn a_walked_root_is_not_its_own_package() {
    let dir = monorepo();
    let tool = dir.path().join("tool");
    let resolver = Resolver::new([tool.clone()]);
    assert_eq!(resolver.context(&tool.join("src")).package, None);
}

#[test]
fn workspace_names_win_over_member_manifests() {
    let dir = tree(&[
        "package.json={\"workspaces\": [\"packages/*\"]}",
        "packages/ui/package.json={\"name\": \"@org/ui\"}",
        "packages/ui/src/a.ts",
    ]);
    let resolver = Resolver::new([dir.path().to_path_buf()]);
    let context = resolver.context(&dir.path().join("packages/ui/src"));
    assert_eq!(context.package.as_deref(), Some("ui"));
}

#[test]
fn excludes_follow_the_innermost_and_root_languages() {
    let dir = monorepo();
    let resolver = Resolver::new([dir.path().to_path_buf()]);
    let engine = resolver.context(&dir.path().join("engine"));
    assert!(engine.language.excludes("target"));
    assert!(!engine.root_language.excludes("target"));
    assert!(resolver.context(&dir.path().join("web")).language.excludes("dist"));
}
