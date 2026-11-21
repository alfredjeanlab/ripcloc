// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! The counting of the files under the given paths into rows by language and kind.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File, FileType};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use ignore::{WalkBuilder, WalkState};

use crate::lang::{Classifier, DirKind, Language};
use crate::project::{Context, Resolver};
use crate::syntax::{self, LineCounts};
use crate::{cfg_test, notebook};

/// The rows of the report, by language name and kind.
pub type Rows = HashMap<(String, Kind), Row>;

/// Whether a row counts source, tests, both of them without attribution, or a format that is not
/// split into either.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Source,
    Test,
    Code,
    Other,
}

/// The files and lines of one row of the report.
#[derive(Default, Clone, Copy, Debug)]
pub struct Stats {
    pub files: usize,
    pub lines: LineCounts,
}

impl Stats {
    pub fn add(&mut self, other: Stats) {
        self.files += other.files;
        self.lines.add(other.lines);
    }

    pub fn line_count(&self) -> usize {
        let LineCounts { blank, comment, code } = self.lines;
        blank + comment + code
    }
}

/// The counts of one language's source or tests, with two breakdowns of them.
#[derive(Default)]
pub struct Row {
    pub stats: Stats,
    /// The share of each package by group, where a package of `None` holds the files outside
    /// every package of its group.
    pub packages: BTreeMap<(Arc<str>, Option<Arc<str>>), Stats>,
    /// The share of each chain of labels, from the top-level directory of a walked root down to
    /// the package of a nested workspace.
    pub chains: BTreeMap<Arc<[Arc<str>]>, Stats>,
}

impl Row {
    /// Add the counts of `other`, which is the same row as another thread counted it.
    fn merge(&mut self, other: Row) {
        self.stats.add(other.stats);
        for (key, stats) in other.packages {
            self.packages.entry(key).or_default().add(stats);
        }
        for (chain, stats) in other.chains {
            self.chains.entry(chain).or_default().add(stats);
        }
    }
}

/// What is counted, and how.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// The size in bytes above which a file is not counted.
    pub max_bytes: u64,
    /// Whether a language's lines are attributed to source or tests. Without it, each language is
    /// one row, and no time is spent on telling tests apart.
    pub attribution: bool,
    /// Whether the directories of dependencies, such as `node_modules`, are counted.
    pub deps: bool,
    /// Whether `.gitignore`, `.ignore` and git's exclude files are respected.
    pub ignore: bool,
    /// How many levels of directories below each path are walked.
    pub max_depth: usize,
}

/// The rows that are counted, with the number of files that could not be read.
#[derive(Default)]
pub struct Counts {
    pub rows: Rows,
    pub unreadable: usize,
}

/// Count the files under `paths`.
///
/// # Errors
///
/// Fails when a path does not exist or cannot be resolved.
pub fn count(paths: &[PathBuf], options: &Options) -> anyhow::Result<Counts> {
    let inputs = inputs(paths)?;
    let resolver = Arc::new(Resolver::new(inputs.iter().map(Input::root)));
    // A few files are counted on this thread, since starting the walker's threads takes longer.
    if inputs.len() <= DIRECT_FILES && inputs.iter().all(Input::is_file) {
        let mut tally = Tally::default();
        for input in &inputs {
            tally.count_file(&input.path, options, &resolver);
        }
        return Ok(Counts { rows: tally.rows, unreadable: tally.unreadable });
    }
    let paths: Vec<PathBuf> = inputs.into_iter().map(|input| input.path).collect();
    Ok(walk(&paths, options, &resolver))
}

/// The most paths that are counted without the walker when every one is a file.
const DIRECT_FILES: usize = 8;

/// A path to count, with its type if it could be read.
struct Input {
    path: PathBuf,
    file_type: Option<FileType>,
}

impl Input {
    fn is_dir(&self) -> bool {
        self.file_type.is_some_and(|t| t.is_dir())
    }

    fn is_file(&self) -> bool {
        self.file_type.is_some_and(|t| t.is_file())
    }

    /// The directory that project files are looked for from, which is the parent of a file.
    fn root(&self) -> PathBuf {
        if self.is_dir() {
            self.path.clone()
        } else {
            self.path.parent().unwrap_or(&self.path).to_path_buf()
        }
    }
}

/// The paths to count, in the order of their canonical forms.
///
/// Paths are compared in their canonical form, so that a path inside another, or one path given
/// twice in different forms, is counted once. A directory is walked as it is given, since a short
/// relative path is faster to open than a long absolute one, but a file is counted at its
/// canonical path, which always has a parent directory to resolve.
fn inputs(paths: &[PathBuf]) -> anyhow::Result<Vec<Input>> {
    let mut paths = paths
        .iter()
        .map(|path| match fs::canonicalize(path) {
            Ok(canonical) => Ok((canonical, path)),
            Err(e) => Err(anyhow::anyhow!("{}: {e}", path.display())),
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    paths.sort_by(|(a, _), (b, _)| a.cmp(b));
    paths.dedup_by(|(a, _), (b, _)| a == b);
    let is_nested = |p: &Path| paths.iter().any(|(q, _)| q != p && p.starts_with(q));
    let inputs = paths
        .iter()
        .filter(|(canonical, _)| !is_nested(canonical))
        .map(|(canonical, given)| {
            let file_type = fs::metadata(canonical).map(|m| m.file_type()).ok();
            let is_dir = file_type.is_some_and(|t| t.is_dir());
            let path = if is_dir { given.to_path_buf() } else { canonical.clone() };
            Input { path, file_type }
        })
        .collect();
    Ok(inputs)
}

/// Count the files under `paths` on the walker's threads.
fn walk(paths: &[PathBuf], options: &Options, resolver: &Arc<Resolver>) -> Counts {
    let Some((first, rest)) = paths.split_first() else { return Counts::default() };
    let mut builder = WalkBuilder::new(first);
    for path in rest {
        builder.add(path);
    }
    if !options.ignore {
        builder.ignore(false).git_ignore(false).git_global(false).git_exclude(false).parents(false);
    }
    let filter_resolver = Arc::clone(resolver);
    let deps = options.deps;
    builder.max_depth(Some(options.max_depth)).filter_entry(move |e| {
        let is_dir = e.file_type().is_some_and(|t| t.is_dir());
        let (Some(parent), true) = (e.path().parent(), is_dir && e.depth() > 0) else {
            return true;
        };
        let name = e.file_name().to_string_lossy();
        let context = filter_resolver.context(parent);
        let skips =
            |language: Language, at_root: bool| language.skips(e.path(), &name, at_root, deps);
        // A `node_modules` is skipped whatever the language of the project that holds it. The
        // directory's own context is resolved last, since that reads it, which is wasted on a
        // directory that its name already skips.
        !(name == "node_modules" && !deps)
            && !skips(context.language, *context.project == *parent)
            && !skips(context.root_language, *context.root == *parent)
            && match filter_resolver.context(e.path()).marker {
                Some(DirKind::Output) => false,
                Some(DirKind::Dependency) => deps,
                None => true,
            }
    });

    // Each thread counts into a tally of its own, which is merged when the walk drops it.
    let rows = Mutex::new(Rows::new());
    let unreadable = AtomicUsize::new(0);
    builder.build_parallel().run(|| {
        let mut local =
            ThreadTally { tally: Tally::default(), rows: &rows, unreadable: &unreadable };
        Box::new(move |entry| {
            // Only regular files are opened, since opening a FIFO could block.
            if let Ok(entry) = entry
                && entry.file_type().is_some_and(|t| t.is_file())
            {
                local.tally.count_file(entry.path(), options, resolver);
            }
            WalkState::Continue
        })
    });
    Counts {
        rows: rows.into_inner().unwrap_or_else(PoisonError::into_inner),
        unreadable: unreadable.into_inner(),
    }
}

/// The tally of one walker thread, which is added to the shared counts when it is dropped.
struct ThreadTally<'a> {
    tally: Tally,
    rows: &'a Mutex<Rows>,
    unreadable: &'a AtomicUsize,
}

impl Drop for ThreadTally<'_> {
    fn drop(&mut self) {
        let mut rows = self.rows.lock().unwrap_or_else(PoisonError::into_inner);
        for (key, row) in std::mem::take(&mut self.tally.rows) {
            rows.entry(key).or_default().merge(row);
        }
        self.unreadable.fetch_add(self.tally.unreadable, Ordering::Relaxed);
    }
}

/// The rows that files are counted into, the number of files that could not be read, and a
/// buffer that is reused to read each file.
#[derive(Default)]
struct Tally {
    rows: Rows,
    unreadable: usize,
    buffer: Vec<u8>,
}

impl Tally {
    /// Count the file at `path`, if it is a text file of a counted type and no larger than
    /// [`Options::max_bytes`], or add one to the unreadable files if it cannot be read.
    fn count_file(&mut self, path: &Path, options: &Options, resolver: &Resolver) {
        let (key, content) = match source(path, options.max_bytes, &mut self.buffer) {
            Ok(Some(source)) => source,
            Ok(None) => return,
            Err(_) => {
                self.unreadable += 1;
                return;
            }
        };
        // A notebook is counted before its context is resolved, since one that is not valid JSON
        // is not counted at all.
        let notebook = match key.as_str() {
            "ipynb" => match notebook::count(&content) {
                Some(counts) => Some(counts),
                None => return,
            },
            _ => None,
        };
        let Some(context) = path.parent().map(|dir| resolver.context(dir)) else { return };
        let lines = || notebook.unwrap_or_else(|| syntax::count_lines(&*content, &key));
        // A file is counted once, in the row that its path decides. Only its lines are divided,
        // because the lines of a Rust source file's inline test modules count as tests.
        let parts = if !syntax::is_split(&key) {
            vec![(Kind::Other, 1, lines())]
        } else if !options.attribution {
            vec![(Kind::Code, 1, lines())]
        } else if is_test(path, &context) {
            vec![(Kind::Test, 1, lines())]
        } else if key == "rs" {
            let (lines, test) = cfg_test::count(&content);
            vec![(Kind::Source, 1, lines.without(test)), (Kind::Test, 0, test)]
        } else {
            vec![(Kind::Source, 1, lines())]
        };
        let language = syntax::language_name(&key);
        for (kind, files, lines) in parts {
            if files > 0 || lines != LineCounts::default() {
                self.add(language, kind, &context, Stats { files, lines });
            }
        }
    }

    /// Add `stats` to the row of `language` and `kind`, and to its breakdowns by the package and
    /// the chain of `context`.
    fn add(&mut self, language: &str, kind: Kind, context: &Context, stats: Stats) {
        let row = self.rows.entry((language.to_string(), kind)).or_default();
        row.stats.add(stats);
        let package = (Arc::clone(&context.group), context.package.clone());
        row.packages.entry(package).or_default().add(stats);
        row.chains.entry(Arc::clone(&context.chain)).or_default().add(stats);
    }
}

/// Whether the file at `path`, whose directory has `context`, is a test.
///
/// A test by the walked root's conventions stays a test inside a nested project, such as a
/// fixture under `tests/`.
fn is_test(path: &Path, context: &Context) -> bool {
    let from = |base: &Path| path.strip_prefix(base).unwrap_or(path);
    Classifier::of(context.root_language).is_test(from(&context.root))
        || Classifier::of(context.language).is_test(from(&context.root))
        || Classifier::of(context.language).is_test(from(&context.project))
}

/// The key of the file at `path` and its text in UTF-8 or another encoding that is compatible
/// with ASCII, if it is a text file of a counted type and no larger than `max_bytes`, or the error
/// if the file cannot be read.
///
/// A file without an extension is keyed by its `#!` line. Coq and Verilog share the extension
/// `.v`, Perl and Prolog share `.pl`, and D shares `.d` with the lists of dependencies that
/// compilers write, so the content tells them apart.
fn source<'a>(
    path: &Path,
    max_bytes: u64,
    buffer: &'a mut Vec<u8>,
) -> io::Result<Option<(String, Cow<'a, [u8]>)>> {
    let mut key = syntax::file_key(path);
    let script = key.is_empty();
    if script && is_never_script(path) || !script && !syntax::is_counted(&key) {
        return Ok(None);
    }
    let Some(content) = read(path, max_bytes, script, buffer)? else { return Ok(None) };
    if script {
        let start = content.get(..SHEBANG_PREFIX).unwrap_or(content);
        let Some(shebang) = syntax::shebang_key(start).filter(|k| syntax::is_counted(k)) else {
            return Ok(None);
        };
        key = shebang.to_string();
    }
    let content = match decode_utf16(content) {
        Some(text) => Cow::Owned(text.into_bytes()),
        None if is_binary(content) => return Ok(None),
        None => Cow::Borrowed(content),
    };
    match key.as_str() {
        "v" => key = syntax::v_key(&content).to_string(),
        "pl" => key = syntax::pl_key(&content).to_string(),
        "d" if syntax::is_make_dependencies(&content) => return Ok(None),
        _ => {}
    }
    Ok(Some((key, content)))
}

/// The content of the regular file at `path`, if it is at most `max_bytes`, or the error if the
/// file cannot be opened or read.
///
/// The size comes from the open file, so a file is stat'ed once, and the content is read up to
/// that size, so a file that grows while it is read is cut at the size it had when opened. With
/// `script`, the rest of the file is read only when its first [`SHEBANG_PREFIX`] bytes start with
/// `#!`, since most files without an extension are not scripts.
fn read<'a>(
    path: &Path,
    max_bytes: u64,
    script: bool,
    buffer: &'a mut Vec<u8>,
) -> io::Result<Option<&'a [u8]>> {
    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    let len = metadata.len();
    if !metadata.is_file() || len > max_bytes {
        return Ok(None);
    }
    // A buffer that held a large file is not kept for the files after it.
    if buffer.capacity() > LARGE_BUFFER {
        *buffer = Vec::new();
    }
    buffer.clear();
    // Reading through `take` stops at `len` without another read call to find the end.
    if script {
        (&mut file).take(len.min(SHEBANG_PREFIX as u64)).read_to_end(buffer)?;
        if !buffer.starts_with(b"#!") {
            return Ok(None);
        }
    }
    let rest = len - buffer.len() as u64;
    let Ok(rest_bytes) = usize::try_from(rest) else { return Ok(None) };
    buffer.reserve_exact(rest_bytes);
    file.take(rest).read_to_end(buffer)?;
    Ok(Some(buffer))
}

/// Whether the file at `path`, which has no extension, is named like a file that is never a
/// script, such as `LICENSE`, so that it is not opened to look for a `#!` line.
///
/// Names match in any case, and an upper case name also matches with a suffix after a hyphen,
/// such as `LICENSE-MIT`.
fn is_never_script(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else { return false };
    let stem = name.split_once('-').map_or("", |(stem, _)| stem);
    NEVER_SCRIPTS.iter().any(|known| name.eq_ignore_ascii_case(known) || stem == *known)
}

/// The upper case names of files without an extension that are never scripts.
const NEVER_SCRIPTS: &[&str] = &[
    "AUTHORS",
    "CHANGELOG",
    "CHANGES",
    "CODEOWNERS",
    "CONTRIBUTORS",
    "COPYING",
    "LICENCE",
    "LICENSE",
    "NOTICE",
    "OWNERS",
    "OWNERS_ALIASES",
    "PATENTS",
    "README",
    "VERSION",
];

/// How many bytes at the start of a file without an extension are read for a `#!` line.
const SHEBANG_PREFIX: usize = 128;

/// Whether `content` looks like a binary file, which is not counted whatever its extension.
///
/// A NUL byte in the first [`BINARY_PREFIX`] bytes marks a binary file, as it does for git.
fn is_binary(content: &[u8]) -> bool {
    memchr::memchr(0, content.get(..BINARY_PREFIX).unwrap_or(content)).is_some()
}

/// The text of `content` in UTF-8, if `content` starts with a UTF-16 byte order mark.
///
/// UTF-16 text has NUL bytes, which would mark it as binary, and its line breaks and comment
/// delimiters are two bytes wide, so it is decoded before it is counted. An unpaired surrogate
/// becomes U+FFFD.
fn decode_utf16(content: &[u8]) -> Option<String> {
    let from_bytes = match content.get(..2)? {
        [0xFF, 0xFE] => u16::from_le_bytes,
        [0xFE, 0xFF] => u16::from_be_bytes,
        _ => return None,
    };
    let (units, _) = content.as_chunks::<2>();
    let chars = char::decode_utf16(units.iter().map(|&pair| from_bytes(pair)));
    Some(chars.map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER)).collect())
}

/// How many bytes at the start of a file are searched for a NUL byte.
const BINARY_PREFIX: usize = 8000;

/// The most bytes that a reused read buffer keeps.
const LARGE_BUFFER: usize = 16 * 1024 * 1024;
