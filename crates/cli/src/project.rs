// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! The project and package that each directory belongs to.
//!
//! - A walked root is a project, whose language is detected with [`Language::detect`].
//! - Below a root, a directory with a manifest, such as `Cargo.toml`, starts a nested project.
//! - Each project's workspace members are packages. A nested project without any is a package
//!   itself, when its manifest names it.
//! - A directory belongs to its innermost project and package, and to the group of the innermost
//!   project that declares packages.
//!
//! Contexts are cached, so each directory is read for manifests once, from any thread.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::lang::{self, Language};
use crate::packages;

/// The project and package of a directory.
pub struct Context {
    /// The walked root that the directory is under.
    pub root: Arc<Path>,
    /// The root of the innermost project.
    pub project: Arc<Path>,
    pub language: Language,
    /// The language of the walked root's project, whose excludes apply to everything below it.
    pub root_language: Language,
    pub package: Option<Arc<str>>,
    /// The path from the walked root to the project that declares the package, or would declare
    /// it, which is empty for the walked root's own packages.
    pub group: Arc<str>,
    /// The labels that a breakdown nests the directory under, one per level: the top-level
    /// directory, then the name of each package that encloses it below that. One of the walked
    /// root's own packages takes the place of the top-level directory.
    pub chain: Arc<[Arc<str>]>,
    /// Whether the directory holds a file that marks it as generated, such as a `pyvenv.cfg`.
    pub generated: bool,
    /// The root and label of the outermost Go module with more than one package that encloses
    /// the directory, whose directories with `.go` files are each a package.
    pub go_module: Option<(Arc<Path>, Arc<str>)>,
}

/// A package's name, and the group of the project that declares it.
type Membership = (Arc<str>, Arc<str>);

/// Resolves and caches the [`Context`] of directories below the walked roots.
pub struct Resolver {
    roots: HashSet<PathBuf>,
    contexts: Mutex<HashMap<PathBuf, Arc<Context>>>,
    /// Package names and their groups by absolute directory, inserted by the project that declares
    /// them.
    packages: Mutex<HashMap<PathBuf, Membership>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Resolver {
    pub fn new(roots: impl IntoIterator<Item = PathBuf>) -> Self {
        Self {
            roots: roots.into_iter().collect(),
            contexts: Mutex::default(),
            packages: Mutex::default(),
        }
    }

    /// The context of `dir`, which is a walked root or below one.
    ///
    /// A directory's parent is always resolved before it, so a project registers its packages
    /// before any directory inside them is resolved.
    pub fn context(&self, dir: &Path) -> Arc<Context> {
        if let Some(context) = lock(&self.contexts).get(dir) {
            return Arc::clone(context);
        }
        let parent = dir.parent().filter(|_| !self.roots.contains(dir)).map(|p| self.context(p));
        let names = lang::names(dir);
        let generated = lang::is_generated(&names);
        let language = match &parent {
            None => Some(Language::detect(dir, &names)),
            Some(_) => Language::detect_nested(&names),
        };
        let root: Arc<Path> = parent.as_ref().map_or_else(|| dir.into(), |p| Arc::clone(&p.root));
        let label: Arc<str> = dir.strip_prefix(&root).unwrap_or(dir).to_string_lossy().into();
        // A Go module declares its packages when it has more than one, and the outermost such
        // module names them.
        let mut declares = language == Some(Language::Go) && packages::go_has_packages(dir);
        let go_module = match parent.as_ref().and_then(|p| p.go_module.clone()) {
            Some(module) => Some(module),
            None if declares => Some((dir.into(), Arc::clone(&label))),
            None => None,
        };
        if let Some((module_root, module_label)) = &go_module
            && names.iter().any(|n| Path::new(n).extension().is_some_and(|e| e == "go"))
            && !dir.strip_prefix(module_root).is_ok_and(|r| r.iter().any(|c| c == "vendor"))
        {
            let name: Arc<str> = if dir == &**module_root {
                ".".into()
            } else {
                dir.file_name().unwrap_or(dir.as_os_str()).to_string_lossy().into()
            };
            lock(&self.packages)
                .entry(dir.to_path_buf())
                .or_insert_with(|| (name, Arc::clone(module_label)));
        }
        if let Some(language) = language {
            let found = packages::detect(dir, language);
            let own_name = (parent.is_some() && found.is_empty() && !declares)
                .then(|| packages::manifest_name(dir, language))
                .flatten();
            declares |= !found.is_empty() || own_name.is_some();
            let mut names = lock(&self.packages);
            for package in found {
                let path =
                    if package.path == "." { dir.to_path_buf() } else { dir.join(&package.path) };
                names.entry(path).or_insert_with(|| (package.name.into(), Arc::clone(&label)));
            }
            if let Some(name) = own_name {
                names.entry(dir.to_path_buf()).or_insert_with(|| (name.into(), Arc::clone(&label)));
            }
        }
        let own = lock(&self.packages).get(dir).cloned();
        let chain: Arc<[Arc<str>]> = match (&parent, &own) {
            (None, _) => Arc::new([]),
            (Some(_), Some((name, group))) if group.is_empty() => Arc::new([Arc::clone(name)]),
            (Some(parent), Some((name, _))) => {
                parent.chain.iter().cloned().chain([Arc::clone(name)]).collect()
            }
            (Some(parent), None) if parent.chain.is_empty() => {
                let name = dir.file_name().unwrap_or(dir.as_os_str()).to_string_lossy();
                Arc::new([name.into()])
            }
            (Some(parent), None) => Arc::clone(&parent.chain),
        };
        let (package, group) = match own {
            Some((name, group)) => (Some(name), group),
            None if declares => (None, label),
            None => match &parent {
                Some(parent) => (parent.package.clone(), Arc::clone(&parent.group)),
                None => (None, label),
            },
        };
        let context = Arc::new(match (&parent, language) {
            (None, language) => {
                let language = language.unwrap_or(Language::Generic);
                Context {
                    root,
                    project: dir.into(),
                    language,
                    root_language: language,
                    package,
                    group,
                    chain,
                    generated,
                    go_module,
                }
            }
            (Some(parent), language) => Context {
                root,
                project: if language.is_some() { dir.into() } else { Arc::clone(&parent.project) },
                language: language.unwrap_or(parent.language),
                root_language: parent.root_language,
                package,
                group,
                chain,
                generated,
                go_module,
            },
        });
        Arc::clone(lock(&self.contexts).entry(dir.to_path_buf()).or_insert(context))
    }
}

#[cfg(test)]
mod tests {
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
        let package = |sub: &str| {
            resolver.context(&dir.path().join(sub)).package.as_deref().map(String::from)
        };
        assert_eq!(package("engine/crates/core/src").as_deref(), Some("engine-core"));
        // A nested project without a named manifest stays in the enclosing package.
        assert_eq!(package("engine/crates/core/python").as_deref(), Some("engine-core"));
        assert_eq!(package("tool/src").as_deref(), Some("the-tool"));
        assert_eq!(package("web/src").as_deref(), Some("web-app"));
        assert_eq!(package("scripts"), None);
        assert_eq!(package("engine"), None);
    }

    #[test]
    fn chains_nest_packages_under_their_top_level_directory() {
        let dir = monorepo();
        let resolver = Resolver::new([dir.path().to_path_buf()]);
        let chain = |sub: &str| -> Vec<String> {
            resolver.context(&dir.path().join(sub)).chain.iter().map(|s| s.to_string()).collect()
        };
        assert_eq!(chain("engine/crates/core/src"), ["engine", "engine-core"]);
        assert_eq!(chain("engine/crates/core/python"), ["engine", "engine-core"]);
        assert_eq!(chain("engine/fixtures"), ["engine"]);
        assert_eq!(chain("tool/src"), ["the-tool"]);
        assert_eq!(chain("scripts"), ["scripts"]);
        assert!(chain("").is_empty());

        // The walked root's own packages are the first level.
        let engine = dir.path().join("engine");
        let resolver = Resolver::new([engine.clone()]);
        let chain = resolver.context(&engine.join("crates/core/src")).chain.to_vec();
        assert_eq!(chain, [Arc::<str>::from("engine-core")]);
    }

    #[test]
    fn packages_are_grouped_by_the_project_that_declares_them() {
        let dir = monorepo();
        let resolver = Resolver::new([dir.path().to_path_buf()]);
        let group = |sub: &str| resolver.context(&dir.path().join(sub)).group.to_string();
        assert_eq!(group("engine/crates/core/src"), "engine");
        assert_eq!(group("engine/crates/core/python"), "engine");
        assert_eq!(group("engine/fixtures"), "engine");
        assert_eq!(group("tool/src"), "tool");
        assert_eq!(group("scripts"), "");
    }

    #[test]
    fn go_directories_are_packages_of_the_outermost_module() {
        let dir = tree(&[
            "go.mod=module example.com/k\\n",
            "main.go",
            "pkg/util/u.go",
            "pkg/util/vendor/v/v.go",
            "staging/api/go.mod=module example.com/api\\n",
            "staging/api/core/c.go",
            "staging/lone/go.mod=module example.com/lone\\n",
        ]);
        let resolver = Resolver::new([dir.path().to_path_buf()]);
        let at = |sub: &str| {
            let context = resolver.context(&dir.path().join(sub));
            (context.package.as_deref().map(String::from), context.group.to_string())
        };
        assert_eq!(at(""), (Some(".".into()), String::new()));
        assert_eq!(at("pkg/util"), (Some("util".into()), String::new()));
        assert_eq!(at("pkg/util/vendor/v"), (Some("util".into()), String::new()));
        assert_eq!(at("staging/api/core"), (Some("core".into()), String::new()));
        assert_eq!(at("staging/lone"), (Some("lone".into()), "staging/lone".into()));
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
        assert!(engine.language.excludes("target", true));
        assert!(!engine.root_language.excludes("target", true));
        assert!(resolver.context(&dir.path().join("web")).language.excludes("dist", true));
    }

    #[test]
    fn a_marker_makes_a_directory_generated() {
        let dir = tree(&["pyproject.toml", "env/pyvenv.cfg", "app/__init__.py"]);
        let resolver = Resolver::new([dir.path().to_path_buf()]);
        assert!(resolver.context(&dir.path().join("env")).generated);
        assert!(!resolver.context(&dir.path().join("app")).generated);
    }
}
