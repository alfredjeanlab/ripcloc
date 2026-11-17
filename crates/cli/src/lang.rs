// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! Project language detection and source/test classification.
//!
//! A file with one of its project language's extensions is classified by that language's test
//! patterns, matched against its path from either the walked root or its project's root. Any other file is classified by [`GENERIC_TESTS`], and is source when none matches.

use std::path::Path;
use std::sync::OnceLock;

use globset::{Glob, GlobSet, GlobSetBuilder};

/// The language of a project, detected from marker files in its root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Rust,
    Go,
    JavaScript,
    Python,
    Ruby,
    Shell,
    Generic,
}

/// Test patterns for files that are not in the project's language.
const GENERIC_TESTS: &[&str] = &[
    "**/tests/**",
    "**/test/**",
    "**/spec/**",
    "**/__tests__/**",
    "**/*_tests/**",
    "**/*_test.*",
    "**/*_tests.*",
    "**/*.test.*",
    "**/*.spec.*",
    "**/test_*.*",
];

impl Language {
    /// Detect the language of a project at `root`, from marker files checked in a fixed order.
    pub fn detect(root: &Path) -> Self {
        let names = names(root);
        let has_sh = |dir: &Path| names_in(dir).any(|n| n.ends_with(".sh"));
        Self::from_manifests(&names)
            .or_else(|| root.join("config/application.rb").exists().then_some(Self::Ruby))
            .or_else(|| {
                (names.iter().any(|n| n.ends_with(".sh"))
                    || has_sh(&root.join("bin"))
                    || has_sh(&root.join("scripts")))
                .then_some(Self::Shell)
            })
            .unwrap_or(Self::Generic)
    }

    /// Detect a project nested in another, which has a manifest such as `Cargo.toml`.
    pub fn detect_nested(dir: &Path) -> Option<Self> {
        Self::from_manifests(&names(dir))
    }

    fn from_manifests(names: &[String]) -> Option<Self> {
        let has = |files: &[&str]| names.iter().any(|n| files.contains(&n.as_str()));
        if has(&["Cargo.toml"]) {
            Some(Self::Rust)
        } else if has(&["go.mod"]) {
            Some(Self::Go)
        } else if has(&["package.json", "tsconfig.json", "jsconfig.json"]) {
            Some(Self::JavaScript)
        } else if has(&["pyproject.toml", "setup.py", "setup.cfg", "requirements.txt"]) {
            Some(Self::Python)
        } else if has(&["Gemfile", "config.ru"]) || names.iter().any(|n| n.ends_with(".gemspec")) {
            Some(Self::Ruby)
        } else {
            None
        }
    }

    fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["rs"],
            Self::Go => &["go"],
            Self::JavaScript => &["js", "jsx", "ts", "tsx", "mjs", "mts", "cjs", "cts"],
            Self::Python => &["py"],
            Self::Ruby => &["rb", "rake"],
            Self::Shell => &["sh", "bash", "bats"],
            Self::Generic => &[],
        }
    }

    fn tests(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &[
                "**/tests/**",
                "**/test/**/*.rs",
                "**/benches/**",
                "**/*_test.rs",
                "**/*_tests.rs",
                "**/*_tests/**",
            ],
            Self::Go => &["**/*_test.go"],
            Self::JavaScript => &[
                "**/*.test.*",
                "**/*.spec.*",
                "**/*_test.*",
                "**/*_tests.*",
                "**/test_*.*",
                "**/__tests__/**",
                "**/*_tests/**",
                "**/test/**",
                "**/tests/**",
            ],
            Self::Python => &[
                "**/tests/**/*.py",
                "**/test/**/*.py",
                "**/test_*.py",
                "**/*_test.py",
                "**/conftest.py",
            ],
            Self::Ruby => {
                &["spec/**/*_spec.rb", "test/**/*_test.rb", "test/**/test_*.rb", "features/**/*.rb"]
            }
            Self::Shell => &["**/*.bats", "**/*_test.sh"],
            Self::Generic => GENERIC_TESTS,
        }
    }

    /// Whether a directory with this name is skipped, such as build output and installed
    /// dependencies. A leading `*` matches any prefix.
    pub fn excludes(self, name: &str) -> bool {
        self.excluded_dirs()
            .iter()
            .any(|p| p.strip_prefix('*').map_or(name == *p, |s| name.ends_with(s)))
    }

    fn excluded_dirs(self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["target"],
            Self::Go => &["vendor"],
            Self::JavaScript => &["node_modules", "dist", "build", ".next", "coverage"],
            Self::Python => &[
                ".venv",
                "venv",
                ".env",
                "env",
                "__pycache__",
                ".mypy_cache",
                ".pytest_cache",
                ".ruff_cache",
                "dist",
                "build",
                "*.egg-info",
                ".tox",
                ".nox",
            ],
            Self::Ruby => &["vendor", "tmp", "log", "coverage"],
            Self::Shell | Self::Generic => &[],
        }
    }
}

/// Classifies paths as source or test.
pub struct Classifier {
    language: Language,
    tests: GlobSet,
    generic_tests: GlobSet,
}

impl Classifier {
    /// The classifier for `language`, built once.
    pub fn of(language: Language) -> &'static Self {
        static ALL: OnceLock<Vec<Classifier>> = OnceLock::new();
        let all = ALL.get_or_init(|| {
            use Language::*;
            [Rust, Go, JavaScript, Python, Ruby, Shell, Generic].map(Classifier::new).into()
        });
        &all[language as usize]
    }

    fn new(language: Language) -> Self {
        Self { language, tests: glob_set(language.tests()), generic_tests: glob_set(GENERIC_TESTS) }
    }

    /// Whether the path is a test, rather than source.
    pub fn is_test(&self, path: &Path) -> bool {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if self.language.extensions().contains(&ext) {
            self.tests.is_match(path)
        } else {
            self.generic_tests.is_match(path)
        }
    }
}

fn names(dir: &Path) -> Vec<String> {
    names_in(dir).collect()
}

fn names_in(dir: &Path) -> impl Iterator<Item = String> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
}

fn glob_set(patterns: &[&str]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for glob in patterns.iter().filter_map(|p| Glob::new(p).ok()) {
        builder.add(glob);
    }
    builder.build().unwrap_or_default()
}

#[cfg(test)]
#[path = "lang_tests.rs"]
mod tests;
