// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! Project language detection and source/test classification.
//!
//! A file with one of its project language's extensions is classified by that language's test
//! patterns, matched against its path from either the walked root or its project's root.
//!
//! Any other file is classified by [`GENERIC_TESTS`], and is source when none matches.
//!
//! Patterns ignore case, so `Test/` and `Foo.Tests/` match as well as `test/` and `foo_tests/`.
//! A path under `src/main/` is always source, because Maven, Gradle and sbt keep tests in
//! `src/test/`, and a `test` directory under `src/main/` is a package such as `org.springframework.test`.

use std::path::Path;
use std::sync::OnceLock;

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

/// The language of a project, detected from marker files in its root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Rust,
    Go,
    JavaScript,
    Python,
    Ruby,
    Dart,
    /// Swift and Objective-C, built by Swift Package Manager or Xcode.
    Swift,
    /// Java, Kotlin and Scala, built by Gradle or Maven.
    Jvm,
    Shell,
    Generic,
}

/// Test patterns for files that are not in the project's language. A `spec` directory alone does
/// not make a test, because outside Ruby it more often holds specifications, such as
/// `rustc_target/src/spec`. A file in it whose name ends in `spec` does, as in RSpec, busted and
/// phpspec.
///
/// A `tb` directory holds a hardware testbench. Hardware projects rarely have a manifest that is
/// recognized, so they are generic, and a language with a manifest has its own patterns for its
/// own files.
const GENERIC_TESTS: &[&str] = &[
    "**/tb/**",
    "**/tests/**",
    "**/test/**",
    "**/__tests__/**",
    "**/*_tests/**",
    "**/*.test/**",
    "**/*.*tests/**",
    "**/*_test.*",
    "**/*_tests.*",
    "**/*.test.*",
    "**/*.spec.*",
    "**/spec/**/*spec.*",
    "**/test_*.*",
];

impl Language {
    /// Detect the language of a project at `root`, whose entries are `names`, from marker files
    /// checked in a fixed order.
    pub fn detect(root: &Path, names: &[String]) -> Self {
        let has_sh = |dir: &Path| names_in(dir).any(|n| n.ends_with(".sh"));
        Self::from_manifests(names)
            .or_else(|| root.join("config/application.rb").exists().then_some(Self::Ruby))
            .or_else(|| {
                (names.iter().any(|n| n.ends_with(".sh"))
                    || has_sh(&root.join("bin"))
                    || has_sh(&root.join("scripts")))
                .then_some(Self::Shell)
            })
            .unwrap_or(Self::Generic)
    }

    /// Detect a project nested in another from the `names` of its directory's entries, which
    /// include a manifest such as `Cargo.toml`.
    pub fn detect_nested(names: &[String]) -> Option<Self> {
        Self::from_manifests(names)
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
        } else if has(&["pubspec.yaml"]) {
            Some(Self::Dart)
        } else if has(&["Package.swift"])
            || names.iter().any(|n| n.ends_with(".xcodeproj") || n.ends_with(".xcworkspace"))
        {
            // An iOS app often has a Gemfile for CocoaPods or fastlane, so Swift is detected
            // before Ruby.
            Some(Self::Swift)
        } else if has(&[
            "build.gradle",
            "build.gradle.kts",
            "settings.gradle",
            "settings.gradle.kts",
            "pom.xml",
        ]) {
            Some(Self::Jvm)
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
            Self::Dart => &["dart"],
            Self::Swift => &["swift", "m", "mm"],
            Self::Jvm => &["java", "kt", "kts", "scala"],
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
                &["spec/**/*.rb", "test/**/*_test.rb", "test/**/test_*.rb", "features/**/*.rb"]
            }
            Self::Dart => {
                &["**/test/**", "**/integration_test/**", "**/test_driver/**", "**/*_test.dart"]
            }
            // Xcode names a test target after the app, as in `AppTests/` and `AppUITests/`, and
            // Swift Package Manager keeps tests in `Tests/`. Both are beside the project's
            // manifest, so a `Sources/ABTests/` module below it is source.
            Self::Swift => &[
                "*Tests/**",
                "**/tests/**",
                "**/*Tests.swift",
                "**/*Test.swift",
                "**/*Tests.m",
                "**/*Tests.mm",
            ],
            // A Gradle source set for tests is named `test` or ends in `Test`, as in `androidTest`
            // and Kotlin Multiplatform's `commonTest`, and `test*` also covers `testFixtures` and
            // `testDebug`.
            Self::Jvm => &["**/src/test*/**", "**/src/*Test/**"],
            Self::Shell => &["**/*.bats", "**/*_test.sh"],
            Self::Generic => GENERIC_TESTS,
        }
    }

    /// Whether the directory `dir` named `name` is skipped, such as build output and installed
    /// dependencies, where `at_root` says whether it is directly inside the project's root.
    /// Dependency directories are counted when `deps` is set.
    ///
    /// A name that only tools use is skipped at any depth, and a name that is also common in
    /// source is skipped only directly inside the project's root, since that is where tools put
    /// it. A name such as `build` that source also uses there is skipped only when the directory
    /// does not directly hold a file with one of the language's extensions, since tools put their
    /// output for these names in subdirectories or in other formats, such as `build/lib/` in
    /// Python and `log/development.log` in Ruby. Only a directory with such a name is read, so
    /// the check costs the rest of the walk nothing.
    pub fn skips(self, dir: &Path, name: &str, at_root: bool, deps: bool) -> bool {
        self.skipped_dirs().iter().any(|SkippedDir(pattern, kind, place)| {
            let matches =
                pattern.strip_prefix('*').map_or(name == *pattern, |suffix| name.ends_with(suffix));
            matches
                && !(deps && *kind == DirKind::Dependency)
                && match place {
                    Place::Anywhere => true,
                    Place::Root => at_root,
                    Place::RootUnlessSource => at_root && !self.holds_source(dir),
                }
        })
    }

    /// Whether `dir` directly holds a file with one of the language's extensions.
    fn holds_source(self, dir: &Path) -> bool {
        let extensions = self.extensions();
        names_in(dir).any(|name| {
            Path::new(&name).extension().is_some_and(|e| extensions.iter().any(|x| e == *x))
        })
    }

    /// The directories that the language's tools write. A leading `*` in a name matches any
    /// prefix.
    fn skipped_dirs(self) -> &'static [SkippedDir] {
        use DirKind::{Dependency, Output};
        use Place::{Anywhere, Root, RootUnlessSource};
        match self {
            Self::Rust => &[SkippedDir("target", Output, Root)],
            Self::Go => &[SkippedDir("vendor", Dependency, Root)],
            // A bundler writes JavaScript straight into `dist/`, and a coverage report has
            // scripts of its own, so their contents cannot tell them from source.
            Self::JavaScript => &[
                SkippedDir("node_modules", Dependency, Anywhere),
                SkippedDir(".next", Output, Anywhere),
                SkippedDir("dist", Output, Root),
                SkippedDir("coverage", Output, Root),
                SkippedDir("build", Output, RootUnlessSource),
            ],
            // A virtual environment is found by its `pyvenv.cfg` rather than by its name, except
            // for `venv`, since virtualenv before version 20 did not write one.
            Self::Python => &[
                SkippedDir(".venv", Dependency, Anywhere),
                SkippedDir("*.egg-info", Dependency, Anywhere),
                SkippedDir("__pycache__", Output, Anywhere),
                SkippedDir(".mypy_cache", Output, Anywhere),
                SkippedDir(".pytest_cache", Output, Anywhere),
                SkippedDir(".ruff_cache", Output, Anywhere),
                SkippedDir(".tox", Output, Anywhere),
                SkippedDir(".nox", Output, Anywhere),
                SkippedDir("venv", Dependency, RootUnlessSource),
                SkippedDir("dist", Output, RootUnlessSource),
                SkippedDir("build", Output, RootUnlessSource),
            ],
            Self::Ruby => &[
                SkippedDir("vendor", Dependency, Root),
                SkippedDir("tmp", Output, RootUnlessSource),
                SkippedDir("log", Output, RootUnlessSource),
                SkippedDir("coverage", Output, RootUnlessSource),
            ],
            Self::Dart => &[SkippedDir("build", Output, RootUnlessSource)],
            Self::Swift => &[
                SkippedDir("Pods", Dependency, Root),
                SkippedDir("Carthage", Dependency, Root),
                SkippedDir("DerivedData", Output, Root),
                SkippedDir("build", Output, RootUnlessSource),
            ],
            Self::Jvm => {
                &[SkippedDir("target", Output, Root), SkippedDir("build", Output, RootUnlessSource)]
            }
            Self::Shell | Self::Generic => &[],
        }
    }
}

/// The name of a directory that a language's tools write, what it holds and where it is skipped.
struct SkippedDir(&'static str, DirKind, Place);

/// What a skipped directory holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirKind {
    /// Installed third-party code, which `--deps` counts.
    Dependency,
    /// Build output or a cache, which is never counted.
    Output,
}

/// Where a directory with a skipped name is skipped.
#[derive(Clone, Copy)]
enum Place {
    Anywhere,
    /// Directly inside the project's root.
    Root,
    /// Directly inside the project's root, when it does not directly hold a source file.
    RootUnlessSource,
}

/// Files whose presence marks what their directory holds, whatever its name and depth. A
/// `CACHEDIR.TAG` follows the Cache Directory Tagging specification, and Cargo, pytest and ruff
/// write one into their output. A `pyvenv.cfg` marks a Python virtual environment.
const MARKERS: &[(&str, DirKind)] =
    &[("CACHEDIR.TAG", DirKind::Output), ("pyvenv.cfg", DirKind::Dependency)];

/// What a directory whose entries are `names` holds, when a marker file in it says so. Output wins
/// over a dependency, since output is never counted.
pub fn marker(names: &[String]) -> Option<DirKind> {
    let found = |kind| MARKERS.iter().any(|(m, k)| *k == kind && names.iter().any(|n| n == m));
    [DirKind::Output, DirKind::Dependency].into_iter().find(|kind| found(*kind))
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
        static RUST: Classifier = Classifier::new(Rust);
        static GO: Classifier = Classifier::new(Go);
        static JAVASCRIPT: Classifier = Classifier::new(JavaScript);
        static PYTHON: Classifier = Classifier::new(Python);
        static RUBY: Classifier = Classifier::new(Ruby);
        static DART: Classifier = Classifier::new(Dart);
        static SWIFT: Classifier = Classifier::new(Swift);
        static JVM: Classifier = Classifier::new(Jvm);
        static SHELL: Classifier = Classifier::new(Shell);
        static GENERIC: Classifier = Classifier::new(Generic);
        match language {
            Rust => &RUST,
            Go => &GO,
            JavaScript => &JAVASCRIPT,
            Python => &PYTHON,
            Ruby => &RUBY,
            Dart => &DART,
            Swift => &SWIFT,
            Jvm => &JVM,
            Shell => &SHELL,
            Generic => &GENERIC,
        }
    }

    const fn new(language: Language) -> Self {
        Self { language, tests: OnceLock::new() }
    }

    fn tests(&self) -> &GlobSet {
        self.tests.get_or_init(|| glob_set(self.language.tests()))
    }

    /// Whether the path is a test, rather than source.
    pub fn is_test(&self, path: &Path) -> bool {
        if is_jvm_main(path) {
            return false;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if self.language.extensions().contains(&ext) {
            self.tests().is_match(path)
        } else {
            // The generic language's own tests are the generic ones.
            Self::of(Language::Generic).tests().is_match(path)
        }
    }
}

/// Whether `path` is under a `src/main/` directory.
fn is_jvm_main(path: &Path) -> bool {
    let mut after_src = false;
    for component in path.components() {
        let name = component.as_os_str();
        if after_src && name == "main" {
            return true;
        }
        after_src = name == "src";
    }
    false
}

/// The names of the entries in `dir`, which are empty when it cannot be read.
pub fn names(dir: &Path) -> Vec<String> {
    names_in(dir).collect()
}

fn names_in(dir: &Path) -> impl Iterator<Item = String> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
}

/// The set of `patterns`, in which `*` matches within one path component and only `**` crosses a
/// `/`, so that `src/*Test/**` does not match a `LoadTest` package deep inside `src/main`.
///
/// A pattern ignores case unless a `*` in it is followed by a capital letter, which marks a
/// camelCase word boundary that only holds with its case. Otherwise `*Test.swift` would match
/// `Latest.swift`.
fn glob_set(patterns: &[&str]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    let glob = |p: &&str| {
        let camel_case =
            p.split('*').skip(1).any(|s| s.starts_with(|c: char| c.is_ascii_uppercase()));
        GlobBuilder::new(p).case_insensitive(!camel_case).literal_separator(true).build().ok()
    };
    for glob in patterns.iter().filter_map(glob) {
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
    pubspec = { "pubspec.yaml", Language::Dart },
    swift_package = { "Package.swift", Language::Swift },
    xcode_project = { "App.xcodeproj/project.pbxproj", Language::Swift },
    xcode_workspace = { "App.xcworkspace/contents.xcworkspacedata", Language::Swift },
    gradle = { "build.gradle", Language::Jvm },
    gradle_kotlin = { "settings.gradle.kts", Language::Jvm },
    maven = { "pom.xml", Language::Jvm },
    root_script = { "build.sh", Language::Shell },
    bin_script = { "bin/run.sh", Language::Shell },
    nothing = { "README.md", Language::Generic },
)]
    fn detects_language_from_marker(marker: &str, expected: Language) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(marker);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "").unwrap();
        assert_eq!(Language::detect(dir.path(), &names(dir.path())), expected);
    }

    #[test]
    fn swift_takes_precedence_over_a_cocoapods_gemfile() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Gemfile"), "").unwrap();
        std::fs::create_dir(dir.path().join("App.xcodeproj")).unwrap();
        assert_eq!(Language::detect(dir.path(), &names(dir.path())), Language::Swift);
    }

    #[test]
    fn cargo_takes_precedence_over_package_json() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        std::fs::write(dir.path().join("package.json"), "").unwrap();
        assert_eq!(Language::detect(dir.path(), &names(dir.path())), Language::Rust);
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
    rb_spec_helper = { Language::Ruby, "spec/spec_helper.rb", true },
    rb_spec_support = { Language::Ruby, "spec/support/helpers.rb", true },
    rb_spec_javascript = { Language::Ruby, "spec/javascripts/a_spec.js", true },
    sh_src = { Language::Shell, "bin/run.sh", false },
    sh_bats = { Language::Shell, "tests/run.bats", true },
    dart_src = { Language::Dart, "lib/main.dart", false },
    dart_test = { Language::Dart, "test/widget_test.dart", true },
    dart_integration = { Language::Dart, "integration_test/app_test.dart", true },
    swift_src = { Language::Swift, "App/ContentView.swift", false },
    swift_xcode_target = { Language::Swift, "AppTests/ContentViewTests.swift", true },
    swift_ui_target = { Language::Swift, "AppUITests/Launch.swift", true },
    swift_package_tests = { Language::Swift, "Tests/CoreTests/Parser.swift", true },
    swift_sources = { Language::Swift, "Sources/Core/Parser.swift", false },
    objc_test = { Language::Swift, "AppTests/ViewTests.m", true },
    swift_module_named_tests = { Language::Swift, "Sources/ABTests/Variant.swift", false },
    swift_word_ending_in_test = { Language::Swift, "Sources/Core/Latest.swift", false },
    swift_word_ending_in_tests = { Language::Swift, "Sources/Core/Contests.swift", false },
    swift_lowercase_tests_dir = { Language::Swift, "tests/a.swift", true },
    jvm_source_set_ending_in_test = { Language::Jvm, "app/src/latest/java/A.java", false },
    jvm_package_named_test = { Language::Jvm, "src/main/java/a/LoadTest/Load.java", false },
    jvm_module_without_manifest = { Language::Jvm, "core/src/test/java/a/ATest.java", true },
    jvm_src = { Language::Jvm, "app/src/main/java/a/Main.java", false },
    jvm_unit = { Language::Jvm, "app/src/test/kotlin/a/MainTest.kt", true },
    jvm_android = { Language::Jvm, "app/src/androidTest/java/a/UiTest.java", true },
    jvm_multiplatform = { Language::Jvm, "shared/src/commonTest/kotlin/a.kt", true },
    jvm_multiplatform_src = { Language::Jvm, "shared/src/commonMain/kotlin/a.kt", false },
    jvm_fixtures = { Language::Jvm, "lib/src/testFixtures/kotlin/a.kt", true },
    jvm_scala = { Language::Jvm, "src/test/scala/a/ASpec.scala", true },
    other_language_in_tests = { Language::Rust, "tests/helper.py", true },
    other_language_src = { Language::Rust, "scripts/gen.py", false },
    generic_spec_dir_is_source = { Language::Generic, "spec/a.c", false },
    generic_dot_spec = { Language::Generic, "src/a.spec.c", true },
    busted_spec = { Language::Generic, "spec/foo_spec.lua", true },
    phpspec_spec = { Language::Generic, "spec/App/FooSpec.php", true },
    ruby_spec_dir = { Language::Ruby, "spec/models/user_spec.rb", true },
    generic_src = { Language::Generic, "src/a.c", false },
    generic_testbench = { Language::Generic, "lib/axis/tb/axis_ep.py", true },
    generic_rtl = { Language::Generic, "lib/axis/rtl/axis_fifo.v", false },
    capitalized_test_dir = { Language::Generic, "src/Compilers/CSharp/Test/A.cs", true },
    capitalized_tests_dir = { Language::Rust, "Tests/a.rs", true },
    dotnet_tests_project = { Language::Generic, "Foo.Tests/B.cs", true },
    dotnet_test_project = { Language::Generic, "src/Foo.Test/B.cs", true },
    dotnet_unit_tests_project = { Language::Generic, "src/Foo.Bar.UnitTests/B.cs", true },
    dotnet_integration_tests_project = { Language::Generic, "Foo.IntegrationTests/B.vb", true },
    dotnet_src_project = { Language::Generic, "src/Foo.Core/B.cs", false },
    capitalized_dot_test_file = { Language::JavaScript, "src/A.Test.ts", true },
    jvm_test = { Language::Generic, "spring-core/src/test/java/a/ATests.java", true },
    jvm_main_test_package = { Language::Generic, "spring-test/src/main/java/org/test/A.java", false },
)]
    fn classifies_paths(language: Language, path: &str, is_test: bool) {
        assert_eq!(Classifier::of(language).is_test(Path::new(path)), is_test);
    }

    #[test]
    fn nested_projects_need_a_manifest() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("build.sh"), "").unwrap();
        assert_eq!(Language::detect_nested(&names(dir.path())), None);
        std::fs::write(dir.path().join("pyproject.toml"), "").unwrap();
        assert_eq!(Language::detect_nested(&names(dir.path())), Some(Language::Python));
    }

    fn skips(language: Language, name: &str, at_root: bool, deps: bool) -> bool {
        let dir = tempfile::tempdir().unwrap();
        language.skips(&dir.path().join(name), name, at_root, deps)
    }

    #[test]
    fn skips_match_names_and_suffixes() {
        assert!(skips(Language::Python, "my_pkg.egg-info", false, false));
        assert!(skips(Language::JavaScript, "node_modules", false, false));
        assert!(!skips(Language::Rust, "src", true, false));
    }

    #[test]
    fn generic_names_are_skipped_only_at_the_root() {
        assert!(skips(Language::Python, "build", true, false));
        assert!(!skips(Language::Python, "build", false, false));
        assert!(!skips(Language::Python, "env", true, false));
        assert!(skips(Language::Rust, "target", true, false));
        assert!(!skips(Language::Rust, "target", false, false));
    }

    #[test]
    fn deps_count_dependencies_but_not_output() {
        assert!(!skips(Language::JavaScript, "node_modules", false, true));
        assert!(!skips(Language::Go, "vendor", true, true));
        assert!(skips(Language::Rust, "target", true, true));
        assert!(skips(Language::Python, "__pycache__", false, true));
    }

    #[test]
    fn a_generic_name_that_holds_source_is_counted() {
        let dir = tempfile::tempdir().unwrap();
        let build = dir.path().join("build");
        std::fs::create_dir_all(build.join("lib")).unwrap();
        assert!(Language::Python.skips(&build, "build", true, false));
        std::fs::write(build.join("core.py"), "").unwrap();
        assert!(!Language::Python.skips(&build, "build", true, false));
        let dist = dir.path().join("dist");
        std::fs::create_dir_all(&dist).unwrap();
        std::fs::write(dist.join("index.js"), "").unwrap();
        assert!(Language::JavaScript.skips(&dist, "dist", true, false));
    }

    #[test]
    fn markers_say_what_a_directory_holds() {
        let names = |n: &[&str]| n.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(marker(&names(&["bin", "lib", "pyvenv.cfg"])), Some(DirKind::Dependency));
        assert_eq!(marker(&names(&["CACHEDIR.TAG", "debug"])), Some(DirKind::Output));
        assert_eq!(marker(&names(&["pyvenv.cfg", "CACHEDIR.TAG"])), Some(DirKind::Output));
        assert_eq!(marker(&names(&["__init__.py", "settings.py"])), None);
    }
}
