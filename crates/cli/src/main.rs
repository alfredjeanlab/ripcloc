// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! `ripcloc`, which quickly counts lines of code by language.

mod cfg_test;
mod cloc;
mod lang;
mod packages;
mod project;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use clap::{Parser, ValueEnum};
use ignore::{WalkBuilder, WalkState};
use serde_json::{Map, Value, json};

use cloc::LineCounts;
use lang::Classifier;
use project::Resolver;

/// Quickly counts lines of code by language
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Files and directories to count
    #[arg(value_name = "PATH", default_value = ".")]
    paths: Vec<PathBuf>,

    /// Maximum directory depth to traverse
    #[arg(long, default_value_t = 100)]
    max_depth: usize,

    /// Levels of breakdown under each language. The first is the top-level directories and the
    /// workspace's own packages, and each level after it lists the packages of a workspace nested
    /// in the level above [default: 1, or 0 for documentation and configuration formats when there
    /// are many languages]
    #[arg(long)]
    depth: Option<usize>,

    /// Count files of any size. Without it, files over 1 GiB are skipped
    #[arg(long, conflicts_with = "no_large")]
    large: bool,

    /// Skip files over 10 MB, such as generated code and data
    #[arg(long, conflicts_with = "large")]
    no_large: bool,

    /// Skip files larger than this many megabytes
    #[arg(long, value_name = "MB", conflicts_with_all = ["large", "no_large"])]
    max_file_size: Option<u64>,

    /// Respect `.gitignore`, `.ignore` and git's exclude files [default]
    #[arg(long, conflicts_with = "no_ignore")]
    ignore: bool,

    /// Count files that `.gitignore`, `.ignore` and git's exclude files leave out. Hidden files
    /// are still skipped, and so are build and dependency directories, such as `target/` directly
    /// inside a Rust project or any directory that holds a `pyvenv.cfg` or `CACHEDIR.TAG`
    #[arg(long, conflicts_with = "ignore")]
    no_ignore: bool,

    /// Output format
    #[arg(short, long, value_enum, default_value_t = Format::Text)]
    output: Format,

    /// Attribute each language's lines to source or tests [default]
    #[arg(long, conflicts_with = "no_attribution")]
    attribution: bool,

    /// Count each language as one row, without telling source from tests, which is faster
    #[arg(long, conflicts_with = "attribution")]
    no_attribution: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    Text,
    Json,
}

/// The version of the JSON report's shape. It changes only when a field is removed or changes
/// meaning, never when counts change or a field is added.
const SCHEMA: u32 = 1;

/// The label of the files outside every package.
const REST: &str = "\u{2026}";

/// The rows of the report, by language name and kind.
type Rows = HashMap<(String, Kind), Row>;

/// Whether a row counts source, tests, both of them with `--no-attribution`, or a format that is not
/// split into either.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Kind {
    Source,
    Test,
    Code,
    Other,
}

/// Counts for one row of the report.
#[derive(Default, Clone, Copy)]
struct Stats {
    files: usize,
    lines: LineCounts,
}

impl Stats {
    fn add(&mut self, other: Stats) {
        self.files += other.files;
        self.lines.add(other.lines);
    }

    fn json(&self) -> Map<String, Value> {
        let LineCounts { blank, comment, code } = self.lines;
        let lines = blank + comment + code;
        let fields = [
            ("files", self.files),
            ("lines", lines),
            ("blank", blank),
            ("comment", comment),
            ("code", code),
        ];
        fields.into_iter().map(|(key, count)| (key.to_string(), json!(count))).collect()
    }

    fn text(&self, label: &str, width: usize) -> String {
        let LineCounts { blank, comment, code } = self.lines;
        let lines = blank + comment + code;
        format!(
            "{label:<width$} {:>5}  {lines:>8}  {blank:>8}  {comment:>8}  {code:>8}",
            self.files
        )
    }
}

/// The counts of one language's source or tests, with two breakdowns of them.
#[derive(Default)]
struct Row {
    stats: Stats,
    /// The share of each package by group, where a package of `None` holds the files outside
    /// every package of its group.
    packages: BTreeMap<(Arc<str>, Option<Arc<str>>), Stats>,
    /// The share of each chain of labels from [`project::Context::chain`].
    chains: BTreeMap<Arc<[Arc<str>]>, Stats>,
}

impl Row {
    /// Add the counts of `other`, the same row as counted by another thread.
    fn merge(&mut self, other: Row) {
        self.stats.add(other.stats);
        for (key, stats) in other.packages {
            self.packages.entry(key).or_default().add(stats);
        }
        for (chain, stats) in other.chains {
            self.chains.entry(chain).or_default().add(stats);
        }
    }

    /// The indented rows under this one, as (depth, label, stats), down to `depth` levels.
    ///
    /// Each level lists the labels below the row above it, then [`REST`] for the files that are
    /// in none of them, so that the rows at each level add up to the row above. A list of one row
    /// repeats the row above it, so it is left out unless `single` keeps it to show where the
    /// row's files are.
    fn breakdown(&self, depth: usize, single: bool) -> Vec<(usize, &str, Stats)> {
        let mut lines = Vec::new();
        self.level(&[], depth, single, &mut lines);
        lines
    }

    fn level<'a>(
        &'a self,
        prefix: &[&'a str],
        depth: usize,
        single: bool,
        lines: &mut Vec<(usize, &'a str, Stats)>,
    ) {
        if prefix.len() >= depth {
            return;
        }
        let mut children: BTreeMap<&str, Stats> = BTreeMap::new();
        let mut rest = Stats::default();
        for (chain, stats) in &self.chains {
            let labels: Vec<&str> = chain.iter().map(|l| &**l).collect();
            if !labels.starts_with(prefix) {
                continue;
            }
            match labels.get(prefix.len()) {
                Some(label) => children.entry(label).or_default().add(*stats),
                None => rest.add(*stats),
            }
        }
        if children.is_empty() || (!single && children.len() + usize::from(rest.files > 0) < 2) {
            return;
        }
        for (label, stats) in children {
            lines.push((prefix.len() + 1, label, stats));
            let mut next = prefix.to_vec();
            next.push(label);
            self.level(&next, depth, single, lines);
        }
        if rest.files > 0 {
            lines.push((prefix.len() + 1, REST, rest));
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Paths are compared in their canonical form, so that a path inside another, or one path
    // given twice in different forms, is counted once. A directory is walked as it is given,
    // since a short relative path is faster to open than a long absolute one, but a file is
    // counted at its canonical path, which always has a parent directory to resolve.
    let mut paths = Vec::new();
    for path in &args.paths {
        let canonical =
            std::fs::canonicalize(path).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
        paths.push((canonical, path));
    }
    paths.sort_by(|(a, _), (b, _)| a.cmp(b));
    paths.dedup_by(|(a, _), (b, _)| a == b);
    let paths: Vec<_> = paths
        .iter()
        .filter(|(p, _)| !paths.iter().any(|(q, _)| q != p && p.starts_with(q)))
        .collect();
    let types: Vec<_> =
        paths.iter().map(|(p, _)| std::fs::metadata(p).map(|m| m.file_type())).collect();
    let paths: Vec<PathBuf> = paths
        .iter()
        .zip(&types)
        .map(|((canonical, given), t)| {
            if t.as_ref().is_ok_and(|t| t.is_dir()) {
                given.to_path_buf()
            } else {
                canonical.clone()
            }
        })
        .collect();
    let roots = paths.iter().zip(&types).map(|(p, t)| {
        if t.as_ref().is_ok_and(|t| t.is_dir()) {
            p.clone()
        } else {
            p.parent().unwrap_or(p).to_path_buf()
        }
    });
    let resolver = Arc::new(Resolver::new(roots));
    const MB: u64 = 1024 * 1024;
    let max_bytes = match args.max_file_size {
        Some(mb) => mb.saturating_mul(MB),
        None if args.large => u64::MAX,
        None if args.no_large => 10 * MB,
        None => 1024 * MB,
    };

    // A few files are counted on this thread, since starting the walker's threads takes longer.
    let rows = if paths.len() <= DIRECT_FILES
        && types.iter().all(|t| t.as_ref().is_ok_and(|t| t.is_file()))
    {
        let (mut rows, mut buffer) = (Rows::new(), Vec::new());
        for path in &paths {
            count_file(path, max_bytes, !args.no_attribution, &resolver, &mut rows, &mut buffer);
        }
        rows
    } else {
        walk(&paths, &args, &resolver, max_bytes)
    };

    // Languages with the most code come first, each with its source before its tests.
    let sorted = sort_rows(&rows);
    let [mut source, mut test, mut code, mut other] = [Stats::default(); 4];
    for ((_, kind), row) in &sorted {
        match kind {
            Kind::Source => &mut source,
            Kind::Test => &mut test,
            Kind::Code => &mut code,
            Kind::Other => &mut other,
        }
        .add(row.stats);
    }
    let mut total = source;
    total.add(test);
    total.add(code);
    total.add(other);

    match args.output {
        Format::Json => {
            let languages: Vec<Value> = sorted
                .iter()
                .map(|((name, kind), row)| {
                    let mut entry = row.stats.json();
                    entry.insert("language".into(), json!(name));
                    entry.insert(
                        "kind".into(),
                        json!(match kind {
                            Kind::Source => "source",
                            Kind::Test => "test",
                            Kind::Code => "code",
                            Kind::Other => "other",
                        }),
                    );
                    if row.packages.keys().any(|(_, name)| name.is_some()) {
                        let packages = row.packages.iter().map(|((group, name), stats)| {
                            let mut package = stats.json();
                            package.insert("name".into(), json!(name.as_deref()));
                            if !group.is_empty() {
                                package.insert("project".into(), json!(group.as_ref()));
                            }
                            package
                        });
                        entry.insert("packages".into(), packages.collect());
                    }
                    Value::Object(entry)
                })
                .collect();
            let totals = if args.no_attribution {
                json!({ "code": code.json(), "other": other.json(), "total": total.json() })
            } else {
                json!({ "source": source.json(), "test": test.json(), "other": other.json(), "total": total.json() })
            };
            let report = json!({ "schema": SCHEMA, "attribution": !args.no_attribution, "languages": languages, "totals": totals });
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Format::Text if sorted.is_empty() => println!("No source files found."),
        Format::Text => {
            // With many languages and no `--depth`, the formats that are not split into source and
            // tests are one number each.
            let languages: HashSet<&str> = rows.keys().map(|(name, _)| name.as_str()).collect();
            let other_depth = match args.depth {
                None if languages.len() >= 4 => 0,
                depth => depth.unwrap_or(1),
            };
            let depth = args.depth.unwrap_or(1);
            // Each line is (depth, label, stats).
            let mut lines = Vec::new();
            // Source is only called source when the language has tests to tell it from.
            let has_tests = |name: &str| rows.contains_key(&(name.to_string(), Kind::Test));
            for ((name, kind), row) in &sorted {
                let label = match kind {
                    Kind::Source if has_tests(name) => format!("{name} (source)"),
                    Kind::Test => format!("{name} (tests)"),
                    Kind::Source | Kind::Code | Kind::Other => name.clone(),
                };
                lines.push((0, label, row.stats));
                let parts = match kind {
                    Kind::Other => row.breakdown(other_depth, false),
                    Kind::Source | Kind::Test | Kind::Code => row.breakdown(depth, true),
                };
                lines.extend(parts.into_iter().map(|(d, l, s)| (d, l.to_string(), s)));
            }
            // The label column is wide enough for the longest label, and at least 25.
            let width =
                lines.iter().map(|(d, l, ..)| 2 * d + l.chars().count()).fold(25, usize::max);
            let rule = "\u{2500}".repeat(width + 47);
            let header = format!(
                "{:<width$} {:>5}  {:>8}  {:>8}  {:>8}  {:>8}",
                "Language", "files", "lines", "blank", "comment", "code"
            );
            println!("{rule}\n{header}\n{rule}");
            for (depth, label, stats) in &lines {
                let indent = "  ".repeat(*depth);
                let line = format!("{indent}{}", stats.text(label, width - indent.len()));
                println!("{line}");
            }
            // Without attribution, code and other formats are only totaled apart when there are both.
            if !args.no_attribution {
                println!(
                    "{rule}\n{}\n{}",
                    source.text("Source total", width),
                    test.text("Test total", width)
                );
                if other.files > 0 {
                    println!("{}", other.text("Other total", width));
                }
            } else if code.files > 0 && other.files > 0 {
                println!(
                    "{rule}\n{}\n{}",
                    code.text("Code total", width),
                    other.text("Other total", width)
                );
            }
            println!("{rule}\n{}\n{rule}", total.text("Total", width));
        }
    }
    Ok(())
}

/// The most paths that are counted without the walker when every one is a file.
const DIRECT_FILES: usize = 8;

/// Count the files under `paths` on the walker's threads.
fn walk(paths: &[PathBuf], args: &Args, resolver: &Arc<Resolver>, max_bytes: u64) -> Rows {
    let Some((first, rest)) = paths.split_first() else { return Rows::new() };
    let mut builder = WalkBuilder::new(first);
    for path in rest {
        builder.add(path);
    }
    let filter_resolver = Arc::clone(resolver);
    if args.no_ignore {
        builder.ignore(false).git_ignore(false).git_global(false).git_exclude(false).parents(false);
    }
    builder.max_depth(Some(args.max_depth)).filter_entry(move |e| {
        let is_dir = e.file_type().is_some_and(|t| t.is_dir());
        let (Some(parent), true) = (e.path().parent(), is_dir && e.depth() > 0) else {
            return true;
        };
        let name = e.file_name().to_string_lossy();
        let context = filter_resolver.context(parent);
        // The directory's own context is resolved last, since that reads it, which is wasted on a
        // directory that its name already excludes.
        name != "node_modules"
            && !context.language.excludes(&name, *context.project == *parent)
            && !context.root_language.excludes(&name, *context.root == *parent)
            && !filter_resolver.context(e.path()).generated
    });

    // Each thread counts into rows of its own, which are merged when the walk drops it.
    let rows = Mutex::new(Rows::new());
    builder.build_parallel().run(|| {
        let mut local = ThreadRows { rows: Rows::new(), buffer: Vec::new(), merged: &rows };
        let attribute = !args.no_attribution;
        Box::new(move |entry| {
            // Only regular files are opened, since opening a FIFO could block.
            if let Ok(entry) = entry
                && entry.file_type().is_some_and(|t| t.is_file())
            {
                count_file(
                    entry.path(),
                    max_bytes,
                    attribute,
                    resolver,
                    &mut local.rows,
                    &mut local.buffer,
                );
            }
            WalkState::Continue
        })
    });
    rows.into_inner().unwrap_or_else(PoisonError::into_inner)
}

/// The rows that one walker thread counts into, and a buffer it reads files into.
struct ThreadRows<'a> {
    rows: Rows,
    buffer: Vec<u8>,
    merged: &'a Mutex<Rows>,
}

impl Drop for ThreadRows<'_> {
    fn drop(&mut self) {
        let mut merged = self.merged.lock().unwrap_or_else(PoisonError::into_inner);
        for (key, row) in std::mem::take(&mut self.rows) {
            merged.entry(key).or_default().merge(row);
        }
    }
}

/// The rows in report order: languages split into source and tests before the formats that are
/// not, each with the most code first, and a language's source before its tests.
fn sort_rows(rows: &Rows) -> Vec<(&(String, Kind), &Row)> {
    let mut code: HashMap<&str, usize> = HashMap::new();
    for ((name, _), row) in rows {
        *code.entry(name).or_default() += row.stats.lines.code;
    }
    let mut sorted: Vec<_> = rows.iter().collect();
    sorted.sort_by(|((a, a_kind), _), ((b, b_kind), _)| {
        (*a_kind == Kind::Other)
            .cmp(&(*b_kind == Kind::Other))
            .then(code.get(b.as_str()).cmp(&code.get(a.as_str())))
            .then(a.cmp(b))
            .then(a_kind.cmp(b_kind))
    });
    sorted
}

/// Count one walked file into `rows`, if it is a text file of a counted type and at most
/// `max_bytes`.
///
/// Without `attribute`, the file's language is one row, and no time is spent on telling tests apart.
fn count_file(
    path: &Path,
    max_bytes: u64,
    attribute: bool,
    resolver: &Resolver,
    rows: &mut Rows,
    buffer: &mut Vec<u8>,
) {
    let mut key = cloc::file_key(path);
    let script = key.is_empty();
    if script && is_never_script(path) || !script && !cloc::is_counted(&key) {
        return;
    }
    // Skip an unreadable file quietly, since warnings would bury the report.
    let Some(content) = read(path, max_bytes, script, buffer) else { return };
    if script {
        let start = content.get(..SHEBANG_PREFIX).unwrap_or(content);
        let Some(shebang) = cloc::shebang_key(start).filter(|k| cloc::is_counted(k)) else {
            return;
        };
        key = shebang.to_string();
    }
    let utf8;
    let content = match decode_utf16(content) {
        Some(text) => {
            utf8 = text;
            utf8.as_bytes()
        }
        None if is_binary(content) => return,
        None => content,
    };
    // Coq and Verilog share the extension `.v`, so the content tells them apart.
    if key == "v" {
        key = cloc::v_key(content).to_string();
    }
    let Some(context) = path.parent().map(|dir| resolver.context(dir)) else { return };
    let from = |base: &Path| path.strip_prefix(base).unwrap_or(path);
    // A test by the walked root's conventions stays a test inside a nested project, such as a
    // fixture under `tests/`.
    let is_test = || {
        Classifier::of(context.root_language).is_test(from(&context.root))
            || Classifier::of(context.language).is_test(from(&context.root))
            || Classifier::of(context.language).is_test(from(&context.project))
    };
    // A file is counted once, in the row that its path decides. Only its lines are divided: the
    // lines of a Rust source file's inline test modules count as tests.
    let kind = |is_test: bool| if is_test { Kind::Test } else { Kind::Source };
    let lines = || cloc::count_lines(content, &key);
    let parts = if !cloc::is_split(&key) {
        vec![(Kind::Other, 1, lines())]
    } else if !attribute {
        vec![(Kind::Code, 1, lines())]
    } else if key == "rs" && !is_test() {
        let (lines, test) = cfg_test::count(content);
        vec![(Kind::Source, 1, lines.without(test)), (Kind::Test, 0, test)]
    } else {
        vec![(kind(is_test()), 1, lines())]
    };

    let language_name = cloc::language_name(&key);
    for (kind, files, lines) in parts {
        if files == 0 && lines == LineCounts::default() {
            continue;
        }
        let stats = Stats { files, lines };
        let row = rows.entry((language_name.to_string(), kind)).or_default();
        row.stats.add(stats);
        let key = (Arc::clone(&context.group), context.package.clone());
        row.packages.entry(key).or_default().add(stats);
        row.chains.entry(Arc::clone(&context.chain)).or_default().add(stats);
    }
}

/// The content of the regular file at `path`, if it is at most `max_bytes`.
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
) -> Option<&'a [u8]> {
    let mut file = File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    let len = metadata.len();
    if !metadata.is_file() || len > max_bytes {
        return None;
    }
    // A buffer that held a large file is not kept for the files after it.
    if buffer.capacity() > LARGE_BUFFER {
        *buffer = Vec::new();
    }
    buffer.clear();
    // Reading through `take` stops at `len` without another read call to find the end.
    if script {
        (&mut file).take(len.min(SHEBANG_PREFIX as u64)).read_to_end(buffer).ok()?;
        if !buffer.starts_with(b"#!") {
            return None;
        }
    }
    let rest = len - buffer.len() as u64;
    buffer.reserve_exact(usize::try_from(rest).ok()?);
    file.take(rest).read_to_end(buffer).ok()?;
    Some(buffer)
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
