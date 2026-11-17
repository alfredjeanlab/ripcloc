// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! The project and package that each directory belongs to.
//!
//! - A walked root is a project, whose language is detected with [`Language::detect`].
//! - Below a root, a directory with a manifest, such as `Cargo.toml`, starts a nested project.
//! - Each project's workspace members are packages. A nested project without any is a package
//!   itself, when its manifest names it.
//! - A directory belongs to its innermost project and package.
//!
//! Contexts are cached, so each directory is read for manifests once, from any thread.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::lang::Language;
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
}

/// Resolves and caches the [`Context`] of directories below the walked roots.
pub struct Resolver {
    roots: HashSet<PathBuf>,
    contexts: Mutex<HashMap<PathBuf, Arc<Context>>>,
    /// Package names by absolute directory, inserted by the project that declares them.
    packages: Mutex<HashMap<PathBuf, Arc<str>>>,
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
        let language = match &parent {
            None => Some(Language::detect(dir)),
            Some(_) => Language::detect_nested(dir),
        };
        if let Some(language) = language {
            let found = packages::detect(dir, language);
            let own_name = (parent.is_some() && found.is_empty())
                .then(|| packages::manifest_name(dir, language))
                .flatten();
            let mut names = lock(&self.packages);
            for package in found {
                let path =
                    if package.path == "." { dir.to_path_buf() } else { dir.join(&package.path) };
                names.entry(path).or_insert_with(|| package.name.into());
            }
            if let Some(name) = own_name {
                names.entry(dir.to_path_buf()).or_insert_with(|| name.into());
            }
        }
        let package = lock(&self.packages)
            .get(dir)
            .cloned()
            .or_else(|| parent.as_ref().and_then(|p| p.package.clone()));
        let context = Arc::new(match (&parent, language) {
            (None, language) => {
                let language = language.unwrap_or(Language::Generic);
                Context {
                    root: dir.into(),
                    project: dir.into(),
                    language,
                    root_language: language,
                    package,
                }
            }
            (Some(parent), language) => Context {
                root: Arc::clone(&parent.root),
                project: if language.is_some() { dir.into() } else { Arc::clone(&parent.project) },
                language: language.unwrap_or(parent.language),
                root_language: parent.root_language,
                package,
            },
        });
        Arc::clone(lock(&self.contexts).entry(dir.to_path_buf()).or_insert(context))
    }
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;
