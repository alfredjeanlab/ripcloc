// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use yare::parameterized;

use super::*;

#[parameterized(
    cargo = { "Cargo.toml", Language::Rust },
    go = { "go.mod", Language::Go },
    npm = { "package.json", Language::JavaScript },
    tsconfig = { "tsconfig.json", Language::JavaScript },
    pyproject = { "pyproject.toml", Language::Python },
    requirements = { "requirements.txt", Language::Python },
    gemfile = { "Gemfile", Language::Ruby },
    gemspec = { "x.gemspec", Language::Ruby },
    root_script = { "build.sh", Language::Shell },
    bin_script = { "bin/run.sh", Language::Shell },
    nothing = { "README.md", Language::Generic },
)]
fn detects_language_from_marker(marker: &str, expected: Language) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(marker);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, "").unwrap();
    assert_eq!(Language::detect(dir.path()), expected);
}

#[test]
fn cargo_takes_precedence_over_package_json() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "").unwrap();
    std::fs::write(dir.path().join("package.json"), "").unwrap();
    assert_eq!(Language::detect(dir.path()), Language::Rust);
}

#[parameterized(
    rust_src = { Language::Rust, "src/lib.rs", false },
    rust_integration = { Language::Rust, "tests/it.rs", true },
    rust_sibling = { Language::Rust, "src/parser_tests.rs", true },
    rust_bench = { Language::Rust, "benches/b.rs", true },
    rust_spec_dir_is_source = { Language::Rust, "spec/x.rs", false },
    go_src = { Language::Go, "pkg/a.go", false },
    go_test = { Language::Go, "pkg/a_test.go", true },
    js_src = { Language::JavaScript, "src/index.ts", false },
    js_dot_test = { Language::JavaScript, "src/a.test.tsx", true },
    js_dunder = { Language::JavaScript, "src/__tests__/a.js", true },
    py_src = { Language::Python, "pkg/mod.py", false },
    py_prefix = { Language::Python, "pkg/test_mod.py", true },
    py_conftest = { Language::Python, "conftest.py", true },
    rb_src = { Language::Ruby, "lib/a.rb", false },
    rb_spec = { Language::Ruby, "spec/a_spec.rb", true },
    sh_src = { Language::Shell, "bin/run.sh", false },
    sh_bats = { Language::Shell, "tests/run.bats", true },
    other_language_in_tests = { Language::Rust, "tests/helper.py", true },
    other_language_src = { Language::Rust, "scripts/gen.py", false },
    generic_spec = { Language::Generic, "spec/a.c", true },
    generic_src = { Language::Generic, "src/a.c", false },
)]
fn classifies_paths(language: Language, path: &str, is_test: bool) {
    assert_eq!(Classifier::of(language).is_test(Path::new(path)), is_test);
}

#[test]
fn nested_projects_need_a_manifest() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("build.sh"), "").unwrap();
    assert_eq!(Language::detect_nested(dir.path()), None);
    std::fs::write(dir.path().join("pyproject.toml"), "").unwrap();
    assert_eq!(Language::detect_nested(dir.path()), Some(Language::Python));
}

#[test]
fn excludes_match_names_and_suffixes() {
    assert!(Language::Python.excludes("my_pkg.egg-info"));
    assert!(Language::JavaScript.excludes("node_modules"));
    assert!(!Language::Rust.excludes("src"));
}
