// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! Behavioral specs for the `ripcloc` binary.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::Path;
use std::process::Command;

/// The binary, with the color settings of the environment cleared, so that output does not depend
/// on the shell that runs the tests.
fn ripcloc() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ripcloc"));
    cmd.env_remove("COLOR").env_remove("NO_COLOR");
    cmd
}

/// A small Cargo project in a temporary directory: `cloc-cmd` is one crate with a source file and a
/// test file, and `cloc-cmd-auto-detect` is a workspace of two crates.
fn fixture(name: &str) -> tempfile::TempDir {
    match name {
        "cloc-cmd" => tree(&[
            (
                "Cargo.toml",
                "[package]\nname = \"cloc-cmd-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            ),
            (
                "src/lib.rs",
                "// A simple module\n\n/// Add two numbers\npub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\n/// Subtract two numbers\npub fn sub(a: i32, b: i32) -> i32 {\n    a - b\n}\n",
            ),
            (
                "tests/basic_test.rs",
                "// Tests for the basic module\n\n#[test]\nfn test_add() {\n    assert_eq!(2 + 2, 4);\n}\n\n#[test]\nfn test_sub() {\n    assert_eq!(4 - 2, 2);\n}\n",
            ),
        ]),
        "cloc-cmd-auto-detect" => tree(&[
            ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"2\"\n"),
            (
                "crates/alpha/Cargo.toml",
                "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            ),
            (
                "crates/alpha/src/lib.rs",
                "// Alpha library\n\n/// Double a value\npub fn double(x: i32) -> i32 {\n    x * 2\n}\n",
            ),
            (
                "crates/beta/Cargo.toml",
                "[package]\nname = \"beta\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            ),
            (
                "crates/beta/src/lib.rs",
                "// Beta library\n\n/// Triple a value\npub fn triple(x: i32) -> i32 {\n    x * 3\n}\n",
            ),
        ]),
        _ => panic!("no fixture named {name}"),
    }
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

/// The text report has a header, a row for each language and kind, a total for each kind and a
/// grand total, each between rules, with every count of the fixture exact
#[test]
fn text_report_counts_the_fixture() {
    let dir = fixture("cloc-cmd");
    let output = ripcloc().current_dir(dir.path()).output().unwrap();
    assert!(output.status.success());
    let rule = "─".repeat(72);
    let expected = [
        rule.as_str(),
        "Language                  files     lines     blank   comment      code",
        &rule,
        "Rust (source)                 1        11         2         3         6",
        "Rust (tests)                  1        11         2         1         8",
        "TOML                          1         4         0         0         4",
        &rule,
        "Total source                  1        11         2         3         6",
        "Total tests                   1        11         2         1         8",
        "Total other                   1         4         0         0         4",
        &rule,
        "Total                         3        26         4         4        18",
        &rule,
    ];
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected.join("\n") + "\n");
}

/// The JSON report has the same counts as the text report, and no `packages` in a project of one
/// package
#[test]
fn json_report_counts_the_fixture() {
    let dir = fixture("cloc-cmd");
    let stats = |files: u64, blank: u64, comment: u64, code: u64| {
        serde_json::json!({
            "files": files,
            "lines": blank + comment + code,
            "blank": blank,
            "comment": comment,
            "code": code,
        })
    };
    let row = |language: &str, kind: &str, files, blank, comment, code| {
        let mut row = stats(files, blank, comment, code);
        row["language"] = language.into();
        row["kind"] = kind.into();
        row
    };
    let expected = serde_json::json!({
        "schema": 1,
        "attribution": true,
        "languages": [
            row("Rust", "source", 1, 2, 3, 6),
            row("Rust", "test", 1, 2, 1, 8),
            row("TOML", "other", 1, 0, 0, 4),
        ],
        "totals": {
            "source": stats(1, 2, 3, 6),
            "test": stats(1, 2, 1, 8),
            "other": stats(1, 0, 0, 4),
            "total": stats(3, 4, 4, 18),
        },
    });
    assert_eq!(cloc_json(dir.path()), expected);
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

/// A directory with a generic name such as `build` or `env` is counted, even directly inside the
/// project's root, when it holds the project's own source
#[test]
fn counts_source_in_dirs_with_generic_names() {
    let temp = tree(&[
        ("pyproject.toml", "[project]\nname = \"x\"\n"),
        ("build/core.py", "x = 1\n"),
        ("env/settings.py", "x = 1\n"),
    ]);
    let json = cloc_json(temp.path());
    assert_eq!(language(&json, "Python", "source")["files"], 2);
}

fn json_with(dir: &Path, args: &[&str]) -> serde_json::Value {
    let output = ripcloc().args(["--output", "json"]).args(args).current_dir(dir).output().unwrap();
    assert!(output.status.success());
    serde_json::from_slice(&output.stdout).unwrap()
}

/// `--deps` counts a virtual environment, which is found by its `pyvenv.cfg`
#[test]
fn deps_count_a_virtual_environment() {
    let temp = tree(&[
        ("pyproject.toml", "[project]\nname = \"x\"\n"),
        ("app.py", "x = 1\n"),
        ("env/pyvenv.cfg", "home = /usr/bin\n"),
        ("env/lib/python3.12/site-packages/dep.py", "x = 1\n"),
    ]);
    let files = |args: &[&str]| {
        language(&json_with(temp.path(), args), "Python", "source")["files"].clone()
    };
    assert_eq!(files(&[]), 1);
    assert_eq!(files(&["--no-deps"]), 1);
    assert_eq!(files(&["--deps"]), 2);
}

/// `--deps` counts `node_modules/` and Go's `vendor/`
#[test]
fn deps_count_node_modules_and_vendor() {
    let temp = tree(&[
        ("package.json", "{\"name\": \"x\"}\n"),
        ("index.js", "export const a = 1;\n"),
        ("node_modules/dep/index.js", "module.exports = 1;\n"),
    ]);
    let files = |args: &[&str]| {
        language(&json_with(temp.path(), args), "JavaScript", "source")["files"].clone()
    };
    assert_eq!(files(&[]), 1);
    assert_eq!(files(&["--deps"]), 2);

    let temp = tree(&[
        ("go.mod", "module example.com/x\n"),
        ("main.go", "package main\n"),
        ("vendor/example.com/dep/dep.go", "package dep\n"),
    ]);
    let files =
        |args: &[&str]| language(&json_with(temp.path(), args), "Go", "source")["files"].clone();
    assert_eq!(files(&[]), 1);
    assert_eq!(files(&["--deps"]), 2);
}

/// Build output such as Rust's `target/` is skipped even with `--deps`
#[test]
fn deps_still_skip_build_output() {
    let temp = tree(&[
        ("Cargo.toml", "[package]\nname = \"x\"\n"),
        ("src/lib.rs", "fn f() {}\n"),
        ("target/debug/build/out.rs", "fn g() {}\n"),
    ]);
    let json = json_with(temp.path(), &["--deps", "--no-ignore"]);
    assert_eq!(language(&json, "Rust", "source")["files"], 1);
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

/// `(source)` only shows beside `(tests)`, and a breakdown of one row, which repeats the row above
/// it, shows only with a `--depth`
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
    // Rust's rows are all in package `a`, so its breakdown would be one row.
    let after = |text: &str| {
        text.lines()
            .skip_while(|l| !l.starts_with("Rust (source)"))
            .nth(1)
            .unwrap_or_default()
            .to_string()
    };
    assert!(!after(&plain).starts_with("  a "), "{plain}");
    let deep = text(&["--depth", "1"]);
    assert!(after(&deep).starts_with("  a "), "{deep}");

    let source_only = tree(&[("x.py", "x = 1\n")]);
    let output = ripcloc().current_dir(source_only.path()).output().unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.lines().any(|l| l.starts_with("Python ")) && !text.contains("(source)"), "{text}");
}

/// Without a `--depth`, other formats are one number each, however few languages there are
#[test]
fn other_formats_are_not_broken_down_by_default() {
    let temp = tree(&[("a/x.rs", "fn a() {}\n"), ("a/x.md", "# a\n"), ("b/x.md", "# b\n")]);
    let text = |args: &[&str]| {
        let output = ripcloc().args(args).current_dir(temp.path()).output().unwrap();
        String::from_utf8(output.stdout).unwrap()
    };
    let after = |text: &str, label: &str| -> String {
        text.lines().skip_while(|l| !l.starts_with(label)).nth(1).unwrap_or_default().to_string()
    };
    let plain = text(&[]);
    assert!(!after(&plain, "Markdown").starts_with("  "), "{plain}");
    assert!(after(&text(&["--depth", "1"]), "Markdown").starts_with("  a "));
}

/// Without a `--depth`, a row with less than 2% of the lines above it is folded into `…`, which
/// still adds up to the row above
#[test]
fn small_rows_are_folded_by_default() {
    let big = "fn a() {}\n".repeat(99);
    let temp =
        tree(&[("big/x.rs", &big), ("small/x.rs", "fn b() {}\n"), ("mid/x.rs", &big[..100])]);
    let text = |args: &[&str]| {
        let output = ripcloc().args(args).current_dir(temp.path()).output().unwrap();
        String::from_utf8(output.stdout).unwrap()
    };
    let plain = text(&[]);
    assert!(plain.lines().any(|l| l.starts_with("  big ")), "{plain}");
    assert!(plain.lines().any(|l| l.starts_with("  mid ")), "{plain}");
    assert!(!plain.contains("small"), "{plain}");
    let rest =
        plain.lines().find(|l| l.starts_with("  \u{2026} ")).unwrap_or_else(|| panic!("{plain}"));
    assert_eq!(rest.split_whitespace().nth(1), Some("1"), "{plain}");
    assert!(text(&["--depth", "1"]).lines().any(|l| l.starts_with("  small ")));
}

/// A reader that stops early, such as `head`, ends the report without an error or a panic
#[test]
fn stops_quietly_when_the_reader_closes() {
    // With `--depth`, no row is folded, so each directory is a row and the report is larger than a
    // pipe's buffer.
    let names: Vec<String> = (0..3000).map(|i| format!("d{i}/x.rs")).collect();
    let files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "fn a() {}\n")).collect();
    let temp = tree(&files);
    let mut child = ripcloc()
        .args(["--depth", "1"])
        .current_dir(temp.path())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut start = [0; 16];
    std::io::Read::read_exact(child.stdout.as_mut().unwrap(), &mut start).unwrap();
    drop(child.stdout.take());
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{:?}", output.status);
    assert!(output.stderr.is_empty(), "{}", String::from_utf8_lossy(&output.stderr));
}

/// The text report is colored only on a terminal or with `COLOR`, `NO_COLOR` wins over `COLOR`, and
/// JSON is never colored
#[test]
fn colors_the_text_report_only_when_asked() {
    let temp = tree(&[("a/x.rs", "fn a() {}\n"), ("b/y.rs", "fn b() {}\n")]);
    let run = |args: &[&str], env: &[(&str, &str)]| {
        let mut cmd = ripcloc();
        cmd.args(args).current_dir(temp.path());
        for (key, value) in env {
            cmd.env(key, value);
        }
        String::from_utf8(cmd.output().unwrap().stdout).unwrap()
    };
    assert!(!run(&[], &[]).contains('\x1b'), "a pipe is not a terminal");
    assert!(run(&[], &[("COLOR", "1")]).contains("\x1b[38;5;74mLanguage"));
    assert!(!run(&[], &[("COLOR", "1"), ("NO_COLOR", "1")]).contains('\x1b'));
    assert!(!run(&["-o", "json"], &[("COLOR", "1")]).contains('\x1b'));
}

/// Help is colored under the same rules as the report, with plain text when it is piped
#[test]
fn colors_help_only_when_asked() {
    let help = |env: &[(&str, &str)]| {
        let mut cmd = ripcloc();
        cmd.arg("--help");
        for (key, value) in env {
            cmd.env(key, value);
        }
        String::from_utf8(cmd.output().unwrap().stdout).unwrap()
    };
    let plain = help(&[]);
    assert!(!plain.contains('\x1b'), "{plain}");
    let colored = help(&[("COLOR", "1")]);
    assert!(colored.contains("\x1b[38;5;74mOptions:\x1b[0m"), "{colored}");
    assert!(colored.contains("\x1b[38;5;250m  -o, --output <FORMAT>\x1b[0m"), "{colored}");
    assert!(colored.contains("\x1b[38;5;245m[default: text]\x1b[0m"), "{colored}");
    assert!(!help(&[("COLOR", "1"), ("NO_COLOR", "1")]).contains('\x1b'));
    // Without its escapes, the colored help is the plain help.
    let stripped = colored.split("\x1b[").enumerate().map(|(i, part)| {
        if i == 0 { part } else { part.split_once('m').map_or(part, |(_, rest)| rest) }
    });
    assert_eq!(stripped.collect::<String>(), plain);
}

/// `--languages` names each language once, even one whose files are counted in two ways
#[test]
fn languages_lists_each_name_once() {
    let output = ripcloc().arg("--languages").output().unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    let names: Vec<&str> = text
        .lines()
        .skip(1)
        .filter(|l| !l.starts_with(' '))
        .filter_map(|l| l.split("  ").next())
        .collect();
    let mut sorted = names.clone();
    sorted.dedup();
    assert_eq!(names, sorted, "{text}");
    let terraform = text.lines().position(|l| l.starts_with("Terraform ")).unwrap();
    assert!(
        text.lines().nth(terraform + 1).is_some_and(|l| l.starts_with(' ') && l.contains(".hcl")),
        "{text}"
    );
}

/// Without a `--depth`, a language with less than 1% of the report's lines is one row, and with a
/// `--depth` it is broken down
#[test]
fn small_languages_are_not_broken_down() {
    let big = "fn a() {}\n".repeat(300);
    let temp =
        tree(&[("a/x.rs", &big), ("b/y.rs", &big), ("c/t.sh", "echo c\n"), ("d/u.sh", "echo d\n")]);
    let text = |args: &[&str]| {
        let output = ripcloc().args(args).current_dir(temp.path()).output().unwrap();
        String::from_utf8(output.stdout).unwrap()
    };
    let plain = text(&[]);
    assert!(plain.lines().any(|l| l.starts_with("  a ")), "{plain}");
    assert!(!plain.lines().any(|l| l.starts_with("  c ") || l.starts_with("  d ")), "{plain}");
    let deep = text(&["--depth", "1"]);
    assert!(
        deep.lines().any(|l| l.starts_with("  c ")) && deep.lines().any(|l| l.starts_with("  d ")),
        "{deep}"
    );
}

/// A missing path is an error
#[test]
fn rejects_missing_paths() {
    let output = ripcloc().args(["does-not-exist"]).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("does-not-exist"));
}

/// In a workspace that is found without configuration, each language row is broken down by the
/// packages of the workspace, and the report has no `packages` of its own
#[test]
fn auto_detect_workspace_packages() {
    let dir = fixture("cloc-cmd-auto-detect");
    let json = cloc_json(dir.path());
    assert!(json.get("packages").is_none(), "{json}");
    let package = |name: &str| {
        serde_json::json!({
            "name": name, "files": 1, "lines": 6, "blank": 1, "comment": 2, "code": 3,
        })
    };
    let rust = language(&json, "Rust", "source");
    assert_eq!(rust["packages"], serde_json::json!([package("alpha"), package("beta")]));
    assert_eq!((rust["files"].clone(), rust["code"].clone()), (2.into(), 6.into()));
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
    let total = text.lines().rev().find(|l| l.starts_with("Total ")).unwrap();
    assert_eq!(total.split_whitespace().nth(1), Some("1"), "{text}");
    let tests = text.lines().find(|l| l.starts_with("Rust (tests)")).unwrap();
    assert_eq!(tests.split_whitespace().nth(2), Some("–"), "{text}");
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
    assert!(!all.contains("Total source") && !all.contains("Total tests"), "{all}");
    assert!(all.lines().any(|l| l.starts_with("Rust ")), "{all}");
    assert!(all.lines().any(|l| l.starts_with("  a ")), "{all}");
    assert!(all.lines().any(|l| l.starts_with("Total code ")), "{all}");
    assert!(all.lines().any(|l| l.starts_with("Total other ")), "{all}");

    let code_only = text(&["--no-attribution", "a/src/lib.rs"]);
    assert!(!code_only.contains("Total code") && !code_only.contains("Total other"), "{code_only}");
    assert!(code_only.lines().any(|l| l.starts_with("Total ")), "{code_only}");

    assert!(text(&["--attribution"]).contains("Rust (tests)"));
}

/// `--compact` is `--no-attribution` without a breakdown, unless a `--depth` asks for one
#[test]
fn compact_is_one_row_per_language() {
    let temp = mixed_tree();
    let run = |args: &[&str]| {
        let output = ripcloc().args(args).current_dir(temp.path()).output().unwrap();
        assert!(output.status.success(), "{args:?}");
        String::from_utf8(output.stdout).unwrap()
    };
    for flag in ["-c", "--compact"] {
        let text = run(&[flag]);
        assert!(!text.contains("(source)") && !text.contains("(tests)"), "{text}");
        assert!(text.lines().all(|l| !l.starts_with("  ")), "{text}");
        assert_eq!(run(&[flag, "-o", "json"]), run(&["--no-attribution", "-o", "json"]));
    }
    assert!(run(&["-c", "--depth", "1"]).lines().any(|l| l.starts_with("  a ")));
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
        &["--deps", "--no-deps"],
        &["--attribution", "--compact"],
        &["--max-file-size", "5", "--large"],
        &["--max-file-size", "5", "--no-large"],
        &["--deps", "--no-deps"],
    ];
    for args in pairs {
        let output =
            ripcloc().args(*args).current_dir(fixture("cloc-cmd").path()).output().unwrap();
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

/// Make each of `files` under `dir` unreadable, and return false if one can still be read, as it
/// can be by root.
#[cfg(unix)]
fn make_unreadable(dir: &Path, files: &[&str]) -> bool {
    use std::os::unix::fs::PermissionsExt;
    for file in files {
        let path = dir.join(file);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::File::open(&path).is_ok() {
            return false;
        }
    }
    true
}

/// When nothing could be counted, the report says how many files could not be read, with the
/// right plural, whether the files are named or walked.
#[cfg(unix)]
#[yare::parameterized(
    named_one = { &["a.rs"], &["a.rs"], "No source files found (skipped 1 unreadable file).\n" },
    walked_many = { &["a.rs", "b.py", "c.go"], &[], "No source files found (skipped 3 unreadable files).\n" },
)]
fn unreadable_files_are_counted_when_nothing_is_found(
    files: &[&str],
    args: &[&str],
    expected: &str,
) {
    let entries: Vec<(&str, &str)> = files.iter().map(|f| (*f, "x = 1\n")).collect();
    let temp = tree(&entries);
    if !make_unreadable(temp.path(), files) {
        return;
    }
    let output = ripcloc().args(args).current_dir(temp.path()).output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
    assert!(output.stderr.is_empty());
}

/// An unreadable file is not mentioned when other files are counted.
#[cfg(unix)]
#[test]
fn unreadable_files_are_quiet_beside_a_report() {
    let temp = tree(&[("a.rs", "fn a() {}\n"), ("b.rs", "fn b() {}\n")]);
    if !make_unreadable(temp.path(), &["b.rs"]) {
        return;
    }
    let output = ripcloc().args(["-o", "json"]).current_dir(temp.path()).output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["totals"]["total"]["files"], 1, "{report}");
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

// =============================================================================
// Command line
// =============================================================================

/// The flags that are accepted but left out of the help and the README.
const HIDDEN: &[&str] = &["--max-depth", "--max-file-size"];

/// Every flag that the parser matches, such as `-o` and `--output`, read from its source.
fn parsed_flags() -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/args.rs");
    let source = std::fs::read_to_string(path).unwrap();
    let longs = source.split("Long(\"").skip(1).filter_map(|s| s.split_once('"'));
    let shorts = source.split("Short('").skip(1).filter_map(|s| s.split_once('\''));
    let mut flags: Vec<String> = longs
        .map(|(name, _)| format!("--{name}"))
        .chain(shorts.map(|(name, _)| format!("-{name}")))
        .collect();
    flags.sort();
    flags.dedup();
    assert!(flags.len() > HIDDEN.len(), "no flags found in args.rs");
    flags
}

/// The flags that `text` mentions, with each `--[no-]x` taken as `--x` and `--no-x`.
fn mentioned_flags(text: &str) -> Vec<String> {
    let mut flags = Vec::new();
    for word in text.split(|c: char| c.is_whitespace() || c == ',' || c == '`') {
        let word = word.trim_end_matches(|c: char| !c.is_ascii_alphanumeric());
        if let Some(name) = word.strip_prefix("--[no-]") {
            flags.extend([format!("--{name}"), format!("--no-{name}")]);
        } else if word.starts_with('-') && word.len() >= 2 && word != "--" {
            flags.push(word.to_string());
        }
    }
    flags.sort();
    flags.dedup();
    flags
}

fn help() -> String {
    let output = ripcloc().arg("--help").output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}

/// The help lists every flag that the parser accepts except the hidden ones, and every flag that
/// it lists is accepted.
#[test]
fn help_lists_every_flag() {
    let parsed = parsed_flags();
    let listed = mentioned_flags(&help());
    for flag in &parsed {
        let hidden = HIDDEN.contains(&flag.as_str());
        assert_eq!(listed.contains(flag), !hidden, "{flag} in the help");
    }
    for flag in &listed {
        assert!(parsed.contains(flag), "{flag} is in the help but is not parsed");
    }
}

/// Every flag in the README's Usage block is accepted, and the hidden flags are not there.
#[test]
fn readme_usage_flags_exist() {
    // The published crate holds the README beside its manifest, and the repository two levels up.
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = [manifest.join("README.md"), manifest.join("../../README.md")]
        .into_iter()
        .find(|p| p.exists())
        .unwrap();
    let readme = std::fs::read_to_string(path).unwrap();
    let (_, usage) = readme.split_once("## Usage").unwrap();
    let (_, usage) = usage.split_once("```").unwrap();
    let (usage, _) = usage.split_once("```").unwrap();
    let parsed = parsed_flags();
    let listed = mentioned_flags(usage);
    assert!(!listed.is_empty());
    for flag in &listed {
        assert!(parsed.contains(flag), "{flag} is in the README but is not parsed");
        assert!(!HIDDEN.contains(&flag.as_str()), "{flag} is hidden but in the README");
    }
}

/// `-h` and `--help` print the same plain text, which fits in 100 columns, even when color is
/// asked for.
#[test]
fn help_is_plain_text() {
    let text = help();
    let short = ripcloc().arg("-h").env("CLICOLOR_FORCE", "1").output().unwrap();
    assert_eq!(String::from_utf8(short.stdout).unwrap(), text);
    assert!(text.is_ascii() && !text.contains('\x1b'));
    assert!(text.lines().all(|l| l.len() < 100), "{text}");
}

/// `-V` and `--version` print the name and the version of the package.
#[test]
fn prints_the_version() {
    for flag in ["-V", "--version"] {
        let output = ripcloc().arg(flag).output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let expected = format!("ripcloc {}\n", env!("CARGO_PKG_VERSION"));
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
}

fn run_in_fixture(args: &[&str]) -> Vec<u8> {
    let output = ripcloc().args(args).current_dir(fixture("cloc-cmd").path()).output().unwrap();
    assert!(output.status.success(), "{args:?}");
    output.stdout
}

/// A value is given after a space, after `=`, or right after a short flag.
#[test]
fn values_take_every_syntax() {
    let json = run_in_fixture(&["-o", "json"]);
    serde_json::from_slice::<serde_json::Value>(&json).unwrap();
    for args in [&["--output", "json"][..], &["--output=json"], &["-ojson"]] {
        assert_eq!(run_in_fixture(args), json, "{args:?}");
    }
    assert_eq!(run_in_fixture(&["-co", "json"]), run_in_fixture(&["-c", "-o", "json"]));
    assert_eq!(run_in_fixture(&["--depth=0"]), run_in_fixture(&["--depth", "0"]));
}

/// A repeated flag is accepted, and its last value wins.
#[test]
fn last_value_wins() {
    let json = run_in_fixture(&["-o", "json"]);
    assert_eq!(run_in_fixture(&["-o", "text", "-o", "json"]), json);
    let depth = run_in_fixture(&["--depth", "1"]);
    assert_eq!(run_in_fixture(&["--depth", "0", "--depth", "1"]), depth);
    assert_eq!(run_in_fixture(&["--no-ignore", "--no-ignore"]), run_in_fixture(&["--no-ignore"]));
}

/// After `--`, every argument is a path, even one that starts with `-`.
#[test]
fn double_dash_ends_the_flags() {
    let temp = tree(&[("-c/x.rs", "fn a() {}\n")]);
    let output = ripcloc().args(["-o", "json", "--", "-c"]).current_dir(temp.path()).output();
    let report: serde_json::Value = serde_json::from_slice(&output.unwrap().stdout).unwrap();
    assert_eq!(report["totals"]["total"]["files"], 1);
    assert_eq!(report["attribution"], true);
}

/// A usage error is printed to stderr with exit status 2 and a message that names the flag.
#[test]
fn usage_errors_name_the_flag() {
    let cases: &[(&[&str], &str)] = &[
        (&["--depth"], "--depth needs a value"),
        (&["-o"], "-o needs a value"),
        (&["--depth", "x"], "invalid value 'x' for --depth"),
        (&["--max-file-size", "-1"], "invalid value '-1' for --max-file-size"),
        (&["--output", "yaml"], "invalid value 'yaml' for --output"),
        (&["--compact=yes"], "--compact does not take a value"),
        (&["--bogus"], "unknown flag --bogus"),
        (&["-x"], "unknown flag -x"),
        (&["--larg"], "did you mean --large?"),
        (&["--no-ignor"], "did you mean --no-ignore?"),
        (&["--large", "--no-large"], "--large cannot be used with --no-large"),
    ];
    for (args, message) in cases {
        let output =
            ripcloc().args(*args).current_dir(fixture("cloc-cmd").path()).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(message), "{args:?}: {stderr}");
        assert!(stderr.contains("--help"), "{args:?}: {stderr}");
        assert!(!stderr.contains('\x1b'), "{args:?}: {stderr}");
    }
}

/// A path that cannot be read fails the run with exit status 1, which is not a usage error.
#[test]
fn run_errors_exit_with_one() {
    let output = ripcloc().arg("does-not-exist").current_dir(fixture("cloc-cmd").path()).output();
    assert_eq!(output.unwrap().status.code(), Some(1));
}

/// The hidden `--max-depth` still limits the walk, and the hidden `--max-file-size` still limits
/// the size of a file, as `skips_files_over_max_file_size` checks.
#[test]
fn hidden_flags_still_work() {
    let temp = tree(&[("top.rs", "fn a() {}\n"), ("a/b/deep.rs", "fn b() {}\n")]);
    let files = |args: &[&str]| {
        let output = ripcloc().args(args).arg("-ojson").current_dir(temp.path()).output().unwrap();
        assert!(output.status.success(), "{args:?}");
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        report["totals"]["total"]["files"].clone()
    };
    assert_eq!(files(&[]), 2);
    assert_eq!(files(&["--max-depth", "1"]), 1);
    assert_eq!(files(&["--max-depth=2", "--max-file-size=1"]), 1);
}

/// `--languages`, `--deps` and `--no-deps` are accepted.
#[test]
fn accepts_languages_and_deps() {
    for flag in ["--languages", "--deps", "--no-deps"] {
        let output = ripcloc().arg(flag).current_dir(fixture("cloc-cmd").path()).output().unwrap();
        assert_eq!(output.status.code(), Some(0), "{flag}");
    }
}

/// A file named like a build script or a shell startup file is counted by its name, a `.d` file
/// that lists a target's dependencies is not D, and a hidden startup file is counted when a path
/// names it, although walking skips it
#[test]
fn counts_files_by_name_and_tells_d_from_dependency_lists() {
    let temp = tree(&[
        ("Jenkinsfile", "// c\npipeline {\n}\n"),
        ("test/groovy/ASpec.groovy", "class ASpec {}\n"),
        ("SConstruct", "# c\nenv = Environment()\n"),
        ("justfile", "build:\n    cargo build\n"),
        ("dotfiles/bashrc", "# c\nalias ll='ls -l'\n"),
        (".zshrc", "export A=1\n"),
        ("src/app.d", "module app;\nvoid main() {}\n"),
        ("obj/app.d", "obj/app.o: src/app.d \\\n  src/b.d\n"),
    ]);
    let json = cloc_json(temp.path());
    assert_eq!(language(&json, "Groovy", "source")["code"], 2);
    assert_eq!(language(&json, "Groovy", "test")["files"], 1);
    assert_eq!(language(&json, "SCons", "other")["code"], 1);
    assert_eq!(language(&json, "Just", "other")["code"], 2);
    assert_eq!(language(&json, "Shell", "source")["files"], 1);
    assert_eq!(language(&json, "D", "source")["files"], 1);
    assert_eq!(json["totals"]["total"]["files"], 6);

    // With a directory among the paths, the files are found by walking.
    for args in [&[".zshrc"][..], &[".zshrc", "src"]] {
        let output =
            ripcloc().args(["-o", "json"]).args(args).current_dir(temp.path()).output().unwrap();
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(language(&json, "Shell", "source")["code"], 1, "{args:?}");
    }
}

/// `--languages` lists every language with its extensions and file names, and whether it is split
/// into source and tests, without counting any path
#[test]
fn lists_languages() {
    let output = ripcloc().args(["--languages", "-o", "json", "no/such/path"]).output().unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let find = |name: &str| {
        let languages = json["languages"].as_array().unwrap();
        languages.iter().find(|l| l["language"] == name).unwrap_or_else(|| panic!("{name}")).clone()
    };
    assert!(find("Rust")["extensions"].as_array().unwrap().contains(&"rs".into()));
    assert_eq!(find("Rust")["kind"], "split");
    assert!(find("Makefile")["names"].as_array().unwrap().contains(&"Makefile".into()));
    assert_eq!(find("Makefile")["kind"], "other");
    assert!(find("Coq")["extensions_by_content"].as_array().unwrap().contains(&"v".into()));

    let output = ripcloc().arg("--languages").output().unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    let row = |name: &str| {
        let row = text.lines().find(|l| l.split("  ").next() == Some(name));
        row.unwrap_or_else(|| panic!("no {name} row in {text}")).to_string()
    };
    assert!(row("Rust").contains(".rs") && row("Rust").contains("source and tests"));
    assert!(row("Groovy").ends_with(".gsh named Jenkinsfile"));
    assert!(row("Shell").contains(".tcsh named .bashrc") && row("Shell").contains(" bashrc"));
    assert!(row("Prolog").contains(".pl (by content)"));
}

/// Meson and Vim script are counted by the names of their files, as well as by extension
#[test]
fn counts_meson_and_vim_files_by_name() {
    let temp = tree(&[
        ("meson.build", "# c\nproject('x', 'c')\n"),
        ("sub/meson_options.txt", "option('a', type: 'boolean')\n"),
        ("meson.options", "option('b', type: 'string')\n"),
        ("vimrc", "\" c\nset nocompatible\n"),
        ("_gvimrc", "set guifont=Mono\n"),
        ("plugin/a.vim", "vim9script\n# c\nvar x = 1\n"),
        ("notes.txt", "not counted\n"),
    ]);
    let json = cloc_json(temp.path());
    let meson = language(&json, "Meson", "other");
    assert_eq!((meson["files"].as_u64(), meson["comment"].as_u64()), (Some(3), Some(1)));
    let vim = language(&json, "Vim Script", "source");
    assert_eq!((vim["files"].as_u64(), vim["comment"].as_u64()), (Some(3), Some(2)));
    assert_eq!(json["totals"]["total"]["files"], 6);
}

/// A `.pl` file is Prolog or Perl by its content
#[test]
fn tells_prolog_from_perl() {
    let temp = tree(&[
        ("prolog/lists.pl", ":- module(lists, [app/3]).\n% c\napp([], L, L).\n"),
        ("prolog/tests/test_lists.pl", ":- use_module(lists).\n"),
        ("scripts/gen.pl", "#!/usr/bin/perl\nuse strict;\nmy $x = 1;\n"),
        ("scripts/short.pl", "1;\n"),
    ]);
    let json = cloc_json(temp.path());
    let prolog = language(&json, "Prolog", "source");
    assert_eq!((prolog["files"].as_u64(), prolog["comment"].as_u64()), (Some(1), Some(1)));
    assert_eq!(language(&json, "Prolog", "test")["files"], 1);
    assert_eq!(language(&json, "Perl", "source")["files"], 2);
}

/// A notebook's cells are counted, without its outputs, and a notebook that is not valid JSON is
/// not counted
#[test]
fn counts_the_cells_of_notebooks() {
    let notebook = r##"{"cells": [
        {"cell_type": "markdown", "metadata": {}, "source": ["# Title\n", "\n", "Prose.\n"]},
        {"cell_type": "code", "metadata": {}, "execution_count": 1,
         "outputs": [{"output_type": "stream", "name": "stdout", "text": ["a\n", "b\n"]}],
         "source": ["# c\n", "x = 1\n", "print(x)\n"]}
    ], "metadata": {"language_info": {"name": "python"}}, "nbformat": 4, "nbformat_minor": 5}"##;
    let temp = tree(&[
        ("analysis.ipynb", notebook),
        ("tests/check.ipynb", notebook),
        ("broken.ipynb", "{\"cells\": [\n"),
    ]);
    let json = cloc_json(temp.path());
    let source = language(&json, "Jupyter Notebook", "source");
    let lines = |row: &serde_json::Value| {
        ["files", "blank", "comment", "code"].map(|k| row[k].as_u64().unwrap())
    };
    assert_eq!(lines(source), [1, 1, 3, 2]);
    assert_eq!(lines(language(&json, "Jupyter Notebook", "test")), [1, 1, 3, 2]);
    assert_eq!(json["totals"]["total"]["files"], 2);
}
