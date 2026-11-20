// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! Behavioral specs for the `ripcloc` binary.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn ripcloc() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ripcloc"))
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

/// `ripcloc` produces a table with Language, files, lines, blank, comment, code columns
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
    assert!(stdout.contains("lines"), "should have lines column");
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

/// A generic name such as `build` is skipped only directly inside the project's root, and a
/// virtual environment or cache is skipped by its marker file whatever its name
#[test]
fn skips_generated_dirs_by_place_and_marker() {
    let temp = tree(&[
        ("pyproject.toml", "[project]\nname = \"x\"\n"),
        ("app/__init__.py", "x = 1\n"),
        ("app/build/core.py", "x = 1\n"),
        ("app/sub/dist/x.py", "x = 1\n"),
        ("env/settings.py", "x = 1\n"),
        ("build/lib/app/__init__.py", "x = 1\n"),
        ("py312/pyvenv.cfg", ""),
        ("py312/lib/site.py", "x = 1\n"),
        ("out/CACHEDIR.TAG", ""),
        ("out/gen.py", "x = 1\n"),
    ]);
    let json = cloc_json(temp.path());
    assert_eq!(language(&json, "Python", "source")["files"], 4);
}

/// A file without an extension is counted in the language that its `#!` line names, and any other
/// file without an extension is not counted
#[test]
fn counts_scripts_by_shebang() {
    let temp = tree(&[
        ("bin/tool", "#!/usr/bin/env python3\nprint(1)\n"),
        ("bin/run", "#!/bin/sh\necho hi\n"),
        ("LICENSE", "MIT\n"),
    ]);

    let json = cloc_json(temp.path());
    assert_eq!(language(&json, "Python", "source")["code"], 1);
    assert_eq!(language(&json, "Shell", "source")["code"], 1);
    assert_eq!(json["totals"]["total"]["files"], 2);
}

/// A `.v` file is Coq or Verilog by its content, and Visual Basic in a `.Tests` project is a test,
/// as C# is
#[test]
fn tells_coq_from_verilog_and_attributes_visual_basic_tests() {
    let temp = tree(&[
        ("theories/A.v", "(* c *)\nFrom Coq Require Import List.\nLemma l : True.\n"),
        ("rtl/top.v", "// c\nmodule top;\nendmodule\n"),
        ("src/Foo/Foo.vbproj", "<Project>\n</Project>\n"),
        ("src/Foo/A.vb", "' c\nModule A\nEnd Module\n"),
        ("src/Foo.Tests/ATests.vb", "Module ATests\nEnd Module\n"),
    ]);
    let json = cloc_json(temp.path());
    let coq = language(&json, "Coq", "source");
    assert_eq!((coq["files"].as_u64(), coq["comment"].as_u64()), (Some(1), Some(1)));
    assert_eq!(language(&json, "Verilog", "source")["code"], 2);
    assert_eq!(language(&json, "Visual Basic", "source")["code"], 2);
    assert_eq!(language(&json, "Visual Basic", "test")["files"], 1);
    assert_eq!(language(&json, "XML", "source")["files"], 1);
}

/// In a project without a manifest, a file in a `tb` directory is part of a testbench
#[test]
fn testbenches_are_tests() {
    let temp = tree(&[
        ("rtl/x.v", "module x;\nendmodule\n"),
        ("tb/eth_ep.py", "class EthFrame:\n    pass\n"),
    ]);
    let json = cloc_json(temp.path());
    assert_eq!(language(&json, "Verilog", "source")["files"], 1);
    assert_eq!(language(&json, "Python", "test")["files"], 1);
    assert_eq!(json["totals"]["test"]["files"], 1);
}

/// A file without an extension that is named like a license or a list of owners is not a script,
/// even with a `#!` line, but a script with another name is counted in full, and a file that is
/// counted by its name, such as `Makefile`, is still counted
#[test]
fn well_known_names_are_not_scripts() {
    let script = format!("#!/bin/sh\n{}", "echo hi\n".repeat(40));
    let temp = tree(&[
        ("OWNERS", "#!/bin/sh\necho owners\n"),
        ("license", "#!/bin/sh\necho license\n"),
        ("LICENSE-MIT", "#!/bin/sh\necho mit\n"),
        ("bin/notice-check", "#!/bin/sh\necho check\n"),
        ("bin/long", &script),
        ("Makefile", "all:\n\techo hi\n"),
    ]);

    let json = cloc_json(temp.path());
    assert_eq!(language(&json, "Shell", "source")["files"], 2);
    assert_eq!(language(&json, "Shell", "source")["code"], 41);
    assert_eq!(language(&json, "Makefile", "other")["code"], 2);
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

/// By default each top-level directory is one row, and `--depth 2` lists the packages in them.
/// Every level adds up to the row above it.
#[test]
fn depth_breaks_directories_down_into_packages() {
    let temp = tree(&[
        ("engine/Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
        ("engine/crates/a/Cargo.toml", "[package]\nname = \"engine-a\"\n"),
        ("engine/crates/a/src/lib.rs", "pub fn a() {}\n"),
        ("engine/crates/b/Cargo.toml", "[package]\nname = \"engine-b\"\n"),
        ("engine/crates/b/src/lib.rs", "pub fn b() {}\npub fn c() {}\n"),
        ("engine/build.rs", "fn main() {}\n"),
        ("tools/nested/gen.rs", "fn main() {}\n"),
        ("gen.rs", "fn main() {}\n"),
    ]);
    let text = |args: &[&str]| {
        let output = ripcloc().args(args).current_dir(temp.path()).output().unwrap();
        String::from_utf8(output.stdout).unwrap()
    };
    // The counts on the line for `label` in the Rust section.
    let row = |text: &str, label: &str| -> Vec<u64> {
        let mut section = text.lines().skip_while(|l| !l.starts_with("Rust "));
        let first = section.next().into_iter();
        let line = first
            .chain(section.take_while(|l| l.starts_with(' ')))
            .find(|l| l.starts_with(label))
            .unwrap_or_else(|| panic!("no {label:?} in\n{text}"));
        line[label.len()..].split_whitespace().map(|n| n.parse().unwrap()).collect()
    };

    let collapsed = text(&[]);
    assert_eq!(row(&collapsed, "Rust ")[0], 5);
    assert_eq!(row(&collapsed, "  engine ")[0], 3);
    assert_eq!(row(&collapsed, "  tools ")[0], 1);
    assert_eq!(row(&collapsed, "  \u{2026} ")[0], 1);
    assert!(!collapsed.contains("engine-a"));

    let expanded = text(&["--depth", "2"]);
    assert_eq!(row(&expanded, "  engine ")[0], 3);
    assert_eq!(row(&expanded, "    engine-a ")[4], 1);
    assert_eq!(row(&expanded, "    engine-b ")[4], 2);
    assert_eq!(row(&expanded, "    \u{2026} ")[0], 1);
    assert_eq!(row(&expanded, "  \u{2026} ")[0], 1);

    let none = text(&["--depth", "0"]);
    assert!(none.lines().all(|l| !l.starts_with("  ")), "{none}");
}

/// The iOS and Android apps inside a React Native project are projects of their own, whose tests
/// follow the conventions of Xcode and Gradle, and whose build directories are skipped
#[test]
fn mobile_apps_have_their_platforms_tests() {
    let temp = tree(&[
        ("package.json", "{\"name\": \"app\"}\n"),
        ("Gemfile", "gem 'cocoapods'\n"),
        ("src/App.tsx", "export const App = 1;\n"),
        ("ios/App.xcodeproj/project.pbxproj", "{}\n"),
        ("ios/Podfile", "platform :ios\n"),
        ("ios/App/AppDelegate.swift", "import UIKit\n"),
        ("ios/AppTests/AppTests.swift", "import XCTest\n"),
        ("ios/AppUITests/Launch.m", "#import <XCTest/XCTest.h>\n"),
        ("ios/Pods/Dep/Dep.swift", "let dep = 1\n"),
        ("android/settings.gradle", "include ':app'\n"),
        ("android/app/build.gradle", "apply plugin: 'com.android.application'\n"),
        ("android/app/src/main/java/a/Main.kt", "class Main\n"),
        ("android/app/src/test/java/a/MainTest.kt", "class MainTest\n"),
        ("android/app/src/androidTest/java/a/UiTest.java", "class UiTest {}\n"),
        ("android/app/build/generated/R.java", "class R {}\n"),
    ]);
    let json = cloc_json(temp.path());
    assert_eq!(language(&json, "Swift", "source")["files"], 1);
    assert_eq!(language(&json, "Swift", "test")["files"], 1);
    assert_eq!(language(&json, "Objective-C", "test")["files"], 1);
    assert_eq!(language(&json, "Kotlin", "source")["files"], 1);
    assert_eq!(language(&json, "Kotlin", "test")["files"], 1);
    assert_eq!(language(&json, "Java", "test")["files"], 1);
    let rows = json["languages"].as_array().unwrap();
    assert!(!rows.iter().any(|l| l["language"] == "Java" && l["kind"] == "source"), "{json}");
}

/// A nested project under the root's test directory is still tests
#[test]
fn nested_projects_keep_the_roots_tests() {
    let temp = tree(&[
        ("Cargo.toml", "[package]\nname = \"root\"\n"),
        ("src/lib.rs", "pub fn a() {}\n"),
        ("tests/fixtures/app/go.mod", "module app\n"),
        ("tests/fixtures/app/main.go", "package main\n"),
    ]);
    let json = cloc_json(temp.path());
    assert_eq!(language(&json, "Go", "test")["files"], 1);
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
    assert_eq!(language(&json, "TOML", "other")["comment"], 1);
}

/// A file given as a path is counted as it is when its directory is the path
#[test]
fn counts_a_file_as_its_directory() {
    let temp = tree(&[
        ("Cargo.toml", "[package]\nname = \"a\"\n"),
        ("src/lib.rs", "fn a() {}\n#[cfg(test)]\nmod tests {\n}\n"),
    ]);
    let count = |path: &str| {
        let output =
            ripcloc().args(["-o", "json", path]).current_dir(temp.path()).output().unwrap();
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };
    let json = count("src/lib.rs");
    assert_eq!(json, count("src"));
    assert_eq!(language(&json, "Rust", "test")["code"], 3);
}

/// A relative path is counted as its absolute path is, with the ignore files above it and the
/// package names of the manifests below it, from the directory itself or from one inside it
#[test]
fn relative_paths_count_as_absolute_paths() {
    let temp = tree(&[
        (".ignore", "skipped.rs\n"),
        ("crates/a/Cargo.toml", "[package]\nname = \"crate-a\"\n"),
        ("crates/a/src/lib.rs", "pub fn a() {}\n"),
        ("crates/a/src/skipped.rs", "fn b() {}\n"),
        ("crates/b/Cargo.toml", "[package]\nname = \"crate-b\"\n"),
        ("crates/b/tests/t.rs", "fn t() {}\n"),
    ]);
    let count = |dir: &Path, path: &Path| {
        let output = ripcloc().args(["-o", "json"]).arg(path).current_dir(dir).output().unwrap();
        assert!(output.status.success());
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };
    let crates = temp.path().join("crates");
    let json = count(temp.path(), &crates);
    assert_eq!(json, count(temp.path(), Path::new("crates")));
    assert_eq!(json, count(temp.path(), Path::new("./crates/")));
    assert_eq!(json, count(&crates, Path::new(".")));
    assert_eq!(json, count(&crates.join("a/src"), Path::new("../..")));
    assert_eq!(language(&json, "Rust", "source")["files"], 1);
    assert_eq!(language(&json, "Rust", "source")["packages"][0]["name"], "crate-a");
    assert_eq!(language(&json, "Rust", "test")["packages"][0]["name"], "crate-b");
}

/// A symlink found while walking is not followed, so its target is not counted twice, but a
/// symlink given as a path is counted
#[cfg(unix)]
#[test]
fn does_not_follow_symlinks_while_walking() {
    let temp = tree(&[("real/x.py", "x = 1\n")]);
    std::os::unix::fs::symlink(temp.path().join("real"), temp.path().join("link")).unwrap();
    std::os::unix::fs::symlink(temp.path().join("real/x.py"), temp.path().join("y.py")).unwrap();
    let json = cloc_json(temp.path());
    assert_eq!(json["totals"]["total"]["files"], 1);
    let output = ripcloc().args(["-o", "json", "link"]).current_dir(temp.path()).output().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["totals"]["total"]["files"], 1);
}

/// Files over `--max-file-size` megabytes are skipped
#[test]
fn skips_files_over_max_file_size() {
    let temp = tree(&[("small.py", "x = 1\n")]);
    std::fs::write(temp.path().join("big.py"), "y = 2\n".repeat(200_000)).unwrap();
    let count = |args: &[&str]| {
        let output =
            ripcloc().args(args).args(["-o", "json"]).current_dir(temp.path()).output().unwrap();
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        json["totals"]["total"]["files"].as_u64().unwrap()
    };
    assert_eq!(count(&[]), 2);
    assert_eq!(count(&["--max-file-size", "1"]), 1);
}

/// Files over 1 GiB are skipped, `--no-large` skips files over 10 MB, and `--large` counts files of
/// any size. `--max-file-size` sets any other limit.
#[test]
fn large_flags_set_the_size_limit() {
    let temp = tree(&[("small.py", "x = 1\n")]);
    std::fs::write(temp.path().join("big.py"), "y = 2\n".repeat(2_000_000)).unwrap();
    // A sparse file is over 1 GiB without taking the space, and is skipped without being read.
    let huge = std::fs::File::create(temp.path().join("huge.py")).unwrap();
    huge.set_len((1 << 30) + 1).unwrap();
    let count = |args: &[&str]| {
        let output =
            ripcloc().args(args).args(["-o", "json"]).current_dir(temp.path()).output().unwrap();
        assert!(output.status.success());
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        json["totals"]["total"]["files"].as_u64().unwrap()
    };
    assert_eq!(count(&[]), 2);
    assert_eq!(count(&["--no-large"]), 1);
    assert_eq!(count(&["--max-file-size", "100"]), 2);
    // `--large` would read the sparse file, so it is removed first.
    std::fs::remove_file(temp.path().join("huge.py")).unwrap();
    assert_eq!(count(&["--large"]), 2);
}

/// Documentation, configuration and build files have one row, wherever they are
#[test]
fn docs_and_config_are_not_split_into_tests() {
    let temp =
        tree(&[("README.md", "# a\n"), ("tests/notes.md", "# b\n"), ("tests/a.toml", "a = 1\n")]);
    let json = cloc_json(temp.path());
    assert_eq!(language(&json, "Markdown", "other")["files"], 2);
    assert_eq!(json["totals"]["other"]["files"], 3);
    assert_eq!(json["totals"]["test"]["files"], 0);
}

/// `(source)` only shows beside `(tests)`, and a breakdown of one row stays for code but not for
/// other formats
#[test]
fn labels_and_single_parts() {
    let temp = tree(&[
        ("Cargo.toml", "[workspace]\nmembers = [\"a\"]\n"),
        ("a/Cargo.toml", "[package]\nname = \"a\"\n"),
        ("a/src/lib.rs", "pub fn a() {}\n"),
        ("a/tests/t.rs", "fn t() {}\n"),
        ("a/src/x.py", "x = 1\n"),
        ("tests/t.py", "def test(): pass\n"),
    ]);
    let text = |args: &[&str]| {
        let output = ripcloc().args(args).current_dir(temp.path()).output().unwrap();
        String::from_utf8(output.stdout).unwrap()
    };
    let plain = text(&[]);
    let labels: Vec<&str> = plain
        .lines()
        .filter(|l| !l.starts_with([' ', '\u{2500}']))
        .map(|l| l.split("  ").next().unwrap())
        .collect();
    assert!(labels.contains(&"Rust (source)"));
    assert!(labels.contains(&"Rust (tests)"));
    assert!(labels.contains(&"Python (source)"));
    assert!(labels.contains(&"Python (tests)"));
    assert!(labels.contains(&"TOML"));
    // Rust's rows are all in package `a`, which still shows where they are.
    let mut after_rust = plain.lines().skip_while(|l| !l.starts_with("Rust (source)"));
    assert!(after_rust.nth(1).unwrap_or_default().starts_with("  a "), "{plain}");

    let source_only = tree(&[("x.py", "x = 1\n")]);
    let output = ripcloc().current_dir(source_only.path()).output().unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.lines().any(|l| l.starts_with("Python ")) && !text.contains("(source)"), "{text}");
}

/// With four or more languages and no `--depth`, other formats are one number each
#[test]
fn other_formats_are_not_broken_down_among_many_languages() {
    let temp = tree(&[
        ("a/x.rs", "fn a() {}\n"),
        ("a/x.py", "x = 1\n"),
        ("b/x.go", "package b\n"),
        ("a/x.md", "# a\n"),
        ("b/x.md", "# b\n"),
    ]);
    let text = |args: &[&str]| {
        let output = ripcloc().args(args).current_dir(temp.path()).output().unwrap();
        String::from_utf8(output.stdout).unwrap()
    };
    let after = |text: &str, label: &str| -> String {
        text.lines().skip_while(|l| !l.starts_with(label)).nth(1).unwrap_or_default().to_string()
    };
    let plain = text(&[]);
    assert!(!after(&plain, "Markdown").starts_with("  "), "{plain}");
    assert!(after(&plain, "Rust").starts_with("  a "), "{plain}");
    assert!(after(&text(&["--depth", "1"]), "Markdown").starts_with("  a "));
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

/// A Rust source file with an inline test module is one source file, and the lines of the module,
/// exactly, are tests
#[test]
fn inline_tests_move_lines_but_not_the_file() {
    let temp = tree(&[(
        "lib.rs",
        "pub fn a() {}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n",
    )]);
    let output =
        ripcloc().args(["-o", "json", "lib.rs"]).current_dir(temp.path()).output().unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let (source, test) = (language(&json, "Rust", "source"), language(&json, "Rust", "test"));
    assert_eq!(
        (source["files"].clone(), source["blank"].clone(), source["code"].clone()),
        (1.into(), 1.into(), 1.into())
    );
    assert_eq!(
        (test["files"].clone(), test["blank"].clone(), test["code"].clone()),
        (0.into(), 0.into(), 5.into())
    );
    assert_eq!(json["totals"]["total"]["files"], 1);

    let output = ripcloc().arg("lib.rs").current_dir(temp.path()).output();
    let text = String::from_utf8(output.unwrap().stdout).unwrap();
    let total = text.lines().find(|l| l.starts_with("Total ")).unwrap();
    assert_eq!(total.split_whitespace().nth(1), Some("1"), "{text}");
}

/// `--no-ignore` counts what ignore files leave out
#[test]
fn no_ignore_counts_ignored_files() {
    let temp = tree(&[
        (".ignore", "skipped.rs\n"),
        ("kept.rs", "fn a() {}\n"),
        ("skipped.rs", "fn b() {}\n"),
    ]);
    let files = |args: &[&str]| {
        let output = ripcloc().args(["-o", "json"]).args(args).current_dir(temp.path()).output();
        let json: serde_json::Value = serde_json::from_slice(&output.unwrap().stdout).unwrap();
        json["totals"]["total"]["files"].as_u64().unwrap()
    };
    assert_eq!(files(&[]), 1);
    assert_eq!(files(&["--no-ignore"]), 2);
    assert_eq!(files(&["--ignore"]), 1);
}

// =============================================================================
// --no-attribution
// =============================================================================

fn mixed_tree() -> tempfile::TempDir {
    tree(&[
        ("Cargo.toml", "[workspace]\nmembers = [\"a\", \"b\"]\n"),
        ("a/Cargo.toml", "[package]\nname = \"a\"\n"),
        ("a/src/lib.rs", "pub fn a() {}\n\n#[cfg(test)]\nmod tests {\n    fn t() {}\n}\n"),
        ("a/tests/t.rs", "fn t() {}\n"),
        ("b/Cargo.toml", "[package]\nname = \"b\"\n"),
        ("b/src/lib.rs", "// b\npub fn b() {}\n"),
        ("README.md", "# x\n"),
    ])
}

/// Every row and total has `lines`, the sum of its blank, comment and code lines, and the report
/// says which schema it follows.
#[test]
fn json_has_lines_and_a_schema() {
    let json = cloc_json(mixed_tree().path());
    assert_eq!(json["schema"], 1);
    let rows = json["languages"].as_array().unwrap().iter();
    let totals = json["totals"].as_object().unwrap().values();
    for stats in rows.chain(totals) {
        let sum =
            ["blank", "comment", "code"].map(|k| stats[k].as_u64().unwrap()).iter().sum::<u64>();
        assert_eq!(stats["lines"].as_u64(), Some(sum), "{stats}");
    }
}

/// With `--no-attribution`, each language is one row of kind `code`, and the totals are of code and
/// other formats
#[test]
fn no_attribution_counts_each_language_as_one_row() {
    let temp = mixed_tree();
    let output =
        ripcloc().args(["-o", "json", "--no-attribution"]).current_dir(temp.path()).output();
    let json: serde_json::Value = serde_json::from_slice(&output.unwrap().stdout).unwrap();
    assert_eq!(json["attribution"], false);
    let rust = language(&json, "Rust", "code");
    assert_eq!(rust["files"], 3);
    assert_eq!(rust["code"], 7);
    assert_eq!(rust["comment"], 1);
    let names: Vec<&str> =
        rust["packages"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["a", "b"]);
    let kinds = json["languages"].as_array().unwrap().iter().map(|l| l["kind"].as_str().unwrap());
    assert!(kinds.into_iter().all(|k| k == "code" || k == "other"), "{json}");
    assert_eq!(language(&json, "Markdown", "other")["files"], 1);

    let totals = &json["totals"];
    assert!(totals.get("source").is_none() && totals.get("test").is_none(), "{totals}");
    assert_eq!(totals["code"]["files"], 3);
    assert_eq!(totals["other"]["files"], 4);
    assert_eq!(totals["total"]["files"], 7);

    // Attribution counts the same files and lines, and says that it attributed them.
    let attributed = cloc_json(temp.path());
    assert_eq!(attributed["attribution"], true);
    assert_eq!(attributed["totals"]["total"], json["totals"]["total"]);
}

/// Text rows have no `(source)` or `(tests)`, and the footer totals code and other formats apart
/// only when there are both
#[test]
fn no_attribution_text_output() {
    let temp = mixed_tree();
    let text = |args: &[&str]| {
        let output = ripcloc().args(args).current_dir(temp.path()).output().unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap()
    };
    let all = text(&["--no-attribution", "--depth", "1"]);
    assert!(!all.contains("(source)") && !all.contains("(tests)"), "{all}");
    assert!(!all.contains("Source total") && !all.contains("Test total"), "{all}");
    assert!(all.lines().any(|l| l.starts_with("Rust ")), "{all}");
    assert!(all.lines().any(|l| l.starts_with("  a ")), "{all}");
    assert!(all.lines().any(|l| l.starts_with("Code total ")), "{all}");
    assert!(all.lines().any(|l| l.starts_with("Other total ")), "{all}");

    let code_only = text(&["--no-attribution", "a/src/lib.rs"]);
    assert!(!code_only.contains("Code total") && !code_only.contains("Other total"), "{code_only}");
    assert!(code_only.lines().any(|l| l.starts_with("Total ")), "{code_only}");

    assert!(text(&["--attribution"]).contains("Rust (tests)"));
}

// =============================================================================
// Errors and plain output
// =============================================================================

/// Contradictory flags are a usage error instead of the last one winning.
#[test]
fn contradictory_flags_are_errors() {
    let pairs: &[&[&str]] = &[
        &["--large", "--no-large"],
        &["--ignore", "--no-ignore"],
        &["--attribution", "--no-attribution"],
        &["--max-file-size", "5", "--large"],
        &["--max-file-size", "5", "--no-large"],
    ];
    for args in pairs {
        let output = ripcloc().args(*args).current_dir(fixture("cloc-cmd")).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?} should be a usage error");
    }
}

/// A named file of a type that is not counted is left out, like such a file in a directory.
#[test]
fn named_uncounted_file_is_ignored() {
    let temp = tree(&[("foo.unknownext", "hello\n"), ("lib.rs", "fn f() {}\n")]);
    let output = ripcloc()
        .args(["-o", "json", "foo.unknownext", "lib.rs"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["totals"]["total"]["files"], 1, "{report}");
}

/// A directory with nothing to count is not an error.
#[test]
fn directory_without_source_is_not_error() {
    let temp = tree(&[("foo.unknownext", "hello\n")]);
    let output = ripcloc().current_dir(temp.path()).output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("No source files found."));
}

/// Output is never colored, even when `COLOR` is set.
#[test]
fn output_has_no_color() {
    let output = ripcloc().env("COLOR", "1").current_dir(fixture("cloc-cmd")).output().unwrap();
    assert!(output.status.success());
    assert!(!output.stdout.contains(&0x1b));
}

/// A file with a NUL byte near its start is binary and is not counted, whatever its extension.
#[test]
fn binary_files_are_skipped() {
    let temp = tree(&[("bin.rs", "fn main() {}\n\0\x01\x02\n"), ("lib.rs", "fn f() {}\n")]);
    let output = ripcloc().args(["-o", "json"]).current_dir(temp.path()).output().unwrap();
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["totals"]["total"]["files"], 1, "{report}");
}

/// A file that starts with a UTF-16 byte order mark is text, and is counted as its UTF-8 decoding.
#[test]
fn utf16_files_are_counted() {
    let temp = tree(&[("lib.rs", "fn f() {}\n")]);
    let text = "// note\nfn main() {}\n";
    let little: Vec<u8> =
        [0xFF, 0xFE].into_iter().chain(text.encode_utf16().flat_map(u16::to_le_bytes)).collect();
    let big: Vec<u8> =
        [0xFE, 0xFF].into_iter().chain(text.encode_utf16().flat_map(u16::to_be_bytes)).collect();
    std::fs::write(temp.path().join("le.rs"), little).unwrap();
    std::fs::write(temp.path().join("be.rs"), big).unwrap();
    let output = ripcloc().args(["-o", "json"]).current_dir(temp.path()).output().unwrap();
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let total = &report["totals"]["total"];
    assert_eq!(
        (&total["files"], &total["comment"], &total["code"]),
        (&3.into(), &2.into(), &3.into()),
        "{report}"
    );
}
