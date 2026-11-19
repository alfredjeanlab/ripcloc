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
    /// The language's test patterns, built on first use, since most runs see few languages.
    tests: OnceLock<GlobSet>,
}

impl Classifier {
    /// The classifier for `language`.
    pub fn of(language: Language) -> &'static Self {
        use Language::*;
        static ALL: [Classifier; 7] = [
            Classifier::new(Rust),
            Classifier::new(Go),
            Classifier::new(JavaScript),
            Classifier::new(Python),
            Classifier::new(Ruby),
            Classifier::new(Shell),
            Classifier::new(Generic),
        ];
        &ALL[language as usize]
    }

    const fn new(language: Language) -> Self {
        Self { language, tests: OnceLock::new() }
    }

    fn tests(&self) -> &GlobSet {
        self.tests.get_or_init(|| glob_set(self.language.tests()))
    }

    /// Whether the path is a test, rather than source.
    pub fn is_test(&self, path: &Path) -> bool {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if self.language.extensions().contains(&ext) {
            self.tests().is_match(path)
        } else {
            // The generic language's own tests are the generic ones.
            Self::of(Language::Generic).tests().is_match(path)
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
mod tests {
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
}
