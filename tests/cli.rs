// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! Behavioral specs for the `ripcloc` binary.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn ripcloc() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("ripcloc"))
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures").join(name)
}

fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    for (path, content) in files {
        let full = temp.path().join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, content).unwrap();
    }
    temp
}

fn language<'a>(json: &'a serde_json::Value, name: &str, kind: &str) -> &'a serde_json::Value {
    json["languages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["language"] == name && l["kind"] == kind)
        .unwrap_or_else(|| panic!("no {name} {kind} row in {json}"))
}

fn cloc_json(dir: &Path) -> serde_json::Value {
    let output = ripcloc().args(["--output", "json"]).current_dir(dir).output().unwrap();
    assert!(output.status.success());
    serde_json::from_slice(&output.stdout).unwrap()
}

// =============================================================================
// Basic output
// =============================================================================

/// `ripcloc` produces a table with Language, files, blank, comment, code columns
#[test]
fn produces_table_output() {
    let mut cmd = ripcloc();
    cmd.current_dir(fixture("cloc-cmd"));
    let output = cmd.output().expect("command should run");
    assert!(output.status.success(), "expected cloc to succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should contain header columns
    assert!(stdout.contains("Language"), "should have Language column");
    assert!(stdout.contains("files"), "should have files column");
    assert!(stdout.contains("blank"), "should have blank column");
    assert!(stdout.contains("comment"), "should have comment column");
    assert!(stdout.contains("code"), "should have code column");
}

/// `ripcloc` shows source and test rows
#[test]
fn splits_source_and_test() {
    let mut cmd = ripcloc();
    cmd.current_dir(fixture("cloc-cmd"));
    let output = cmd.output().expect("command should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("(source)"), "should have source rows");
    assert!(stdout.contains("(tests)"), "should have test rows");
}

/// `ripcloc` shows summary totals
#[test]
fn shows_totals() {
    let mut cmd = ripcloc();
    cmd.current_dir(fixture("cloc-cmd"));
    let output = cmd.output().expect("command should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Source total"), "should have Source total");
    assert!(stdout.contains("Test total"), "should have Test total");
    assert!(stdout.contains("Total"), "should have grand Total");
}

// =============================================================================
// JSON output
// =============================================================================

/// `ripcloc --output json` produces valid JSON
#[test]
fn json_output_is_valid() {
    let mut cmd = ripcloc();
    cmd.args(["--output", "json"]);
    cmd.current_dir(fixture("cloc-cmd"));
    let output = cmd.output().expect("command should run");
    assert!(output.status.success());
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("should be valid JSON");
    assert!(json.get("languages").is_some(), "should have languages array");
    assert!(json.get("totals").is_some(), "should have totals object");
}

/// JSON output contains language entries with expected fields
#[test]
fn json_has_language_entries() {
    let mut cmd = ripcloc();
    cmd.args(["--output", "json"]);
    cmd.current_dir(fixture("cloc-cmd"));
    let output = cmd.output().expect("command should run");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let languages = json["languages"].as_array().unwrap();
    assert!(!languages.is_empty(), "should have language entries");

    // Each entry should have required fields
    for lang in languages {
        assert!(lang.get("language").is_some(), "should have language field");
        assert!(lang.get("kind").is_some(), "should have kind field");
        assert!(lang.get("files").is_some(), "should have files field");
        assert!(lang.get("blank").is_some(), "should have blank field");
        assert!(lang.get("comment").is_some(), "should have comment field");
        assert!(lang.get("code").is_some(), "should have code field");
    }
}

/// JSON totals contain source, test, and total sections
#[test]
fn json_totals_structure() {
    let mut cmd = ripcloc();
    cmd.args(["--output", "json"]);
    cmd.current_dir(fixture("cloc-cmd"));
    let output = cmd.output().expect("command should run");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let totals = &json["totals"];
    assert!(totals.get("source").is_some(), "should have source totals");
    assert!(totals.get("test").is_some(), "should have test totals");
    assert!(totals.get("total").is_some(), "should have grand total");
}

// =============================================================================
// File classification
// =============================================================================

/// Source/test split matches adapter classification
#[test]
fn fixture_counts() {
    let mut cmd = ripcloc();
    cmd.args(["--output", "json"]);
    cmd.current_dir(fixture("cloc-cmd"));
    let output = cmd.output().expect("command should run");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let totals = &json["totals"];

    // Fixture has src/lib.rs (source) and tests/basic_test.rs (test)
    let source_files = totals["source"]["files"].as_u64().unwrap();
    let test_files = totals["test"]["files"].as_u64().unwrap();
    assert!(source_files >= 1, "should have at least 1 source file");
    assert!(test_files >= 1, "should have at least 1 test file");
}

// =============================================================================
// Language defaults
// =============================================================================

/// Language-specific build and dependency directories are not counted
#[test]
fn skips_language_default_dirs() {
    let temp = tempfile::tempdir().unwrap();
    let write = |path: &str, content: &str| {
        let full = temp.path().join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, content).unwrap();
    };
    write("package.json", "{\"name\": \"x\"}\n");
    write("src/index.js", "export const a = 1;\n");
    write("node_modules/dep/index.js", "module.exports = 1;\n");
    write("dist/bundle.js", "var a = 1;\n");

    let json = cloc_json(temp.path());
    assert_eq!(language(&json, "JavaScript", "source")["files"], 1);
}

/// A monorepo without a root manifest is broken down by the packages of its nested projects
#[test]
fn breaks_down_nested_projects() {
    let temp = tree(&[
        ("engine/Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
        ("engine/crates/core/Cargo.toml", "[package]\nname = \"engine-core\"\n"),
        ("engine/crates/core/src/lib.rs", "pub fn a() {}\n"),
        ("engine/target/debug/gen.rs", "fn generated() {}\n"),
        ("web/package.json", "{\"name\": \"web-app\"}\n"),
        ("web/src/index.ts", "export const a = 1;\n"),
        ("web/src/index.test.ts", "test();\n"),
        ("web/dist/bundle.js", "var a = 1;\n"),
    ]);
    let json = cloc_json(temp.path());
    let names = |lang: &str, kind: &str| -> Vec<String> {
        language(&json, lang, kind)["packages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["name"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(language(&json, "Rust", "source")["files"], 1, "target/ is skipped");
    assert_eq!(names("Rust", "source"), ["engine-core"]);
    assert_eq!(names("TypeScript", "source"), ["web-app"]);
    assert_eq!(names("TypeScript", "test"), ["web-app"]);
    assert!(json["languages"].as_array().unwrap().iter().all(|l| l["language"] != "JavaScript"));
}

/// Several paths are counted together, and a path inside another is counted once
#[test]
fn counts_several_paths_once() {
    let temp = tree(&[("a/x.py", "x = 1\n"), ("b/y.py", "y = 1\n"), ("b/z.toml", "# c\nz = 1\n")]);
    let output = ripcloc()
        .args(["-o", "json", "a", "b", "b/y.py", "a"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(language(&json, "Python", "source")["files"], 2);
    assert_eq!(language(&json, "TOML", "source")["comment"], 1);
}

/// A missing path is an error
#[test]
fn rejects_missing_paths() {
    let output = ripcloc().args(["does-not-exist"]).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("does-not-exist"));
}

/// Packages are omitted from language entries when no packages configured
#[test]
fn json_omits_packages_when_unconfigured() {
    let mut cmd = ripcloc();
    cmd.args(["--output", "json"]);
    cmd.current_dir(fixture("cloc-cmd"));
    let output = cmd.output().expect("command should run");
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    // cloc-cmd fixture has no packages configured and is a single crate
    assert!(json.get("packages").is_none(), "should not have top-level packages");
    // No language entry should have packages either
    let languages = json["languages"].as_array().unwrap();
    for lang in languages {
        assert!(
            lang.get("packages").is_none(),
            "language entry should not have packages when unconfigured"
        );
    }
}

/// Auto-detected Rust workspace shows package breakdown inside language entries
#[test]
fn auto_detect_workspace_packages() {
    let mut cmd = ripcloc();
    cmd.args(["--output", "json"]);
    cmd.current_dir(fixture("cloc-cmd-auto-detect"));
    let output = cmd.output().expect("command should run");
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    // No top-level packages
    assert!(json.get("packages").is_none(), "should not have top-level packages");
    // Packages are nested inside language entries
    let languages = json["languages"].as_array().unwrap();
    let pkg_names: Vec<String> = languages
        .iter()
        .flat_map(|l| {
            l.get("packages")
                .and_then(|p| p.as_array())
                .into_iter()
                .flatten()
                .filter_map(|p| p.get("name").and_then(|n| n.as_str()).map(String::from))
        })
        .collect();
    assert!(pkg_names.contains(&"alpha".to_string()), "should have alpha package");
    assert!(pkg_names.contains(&"beta".to_string()), "should have beta package");
}
