// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! `ripcloc`, which counts lines of code by language, split into source and tests.

mod cfg_test;
mod cloc;
mod lang;
mod packages;
mod project;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use clap::{Parser, ValueEnum};
use ignore::{WalkBuilder, WalkState};
use serde_json::{Value, json};

use cloc::LineCounts;
use lang::Classifier;
use project::Resolver;

/// Counts lines of code by language, split into source and tests
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
    #[arg(long, overrides_with = "no_large")]
    large: bool,

    /// Skip files over 10 MB, such as generated code and data
    #[arg(long, overrides_with = "large")]
    no_large: bool,

    /// Skip files larger than this many megabytes, whether or not `--large` or `--no-large` is
    /// given
    #[arg(long, value_name = "MB")]
    max_file_size: Option<u64>,

    /// Respect `.gitignore`, `.ignore` and git's exclude files [default]
    #[arg(long, overrides_with = "no_ignore")]
    ignore: bool,

    /// Count files that `.gitignore`, `.ignore` and git's exclude files leave out. Hidden files
    /// and build and dependency directories are still skipped
    #[arg(long, overrides_with = "ignore")]
    no_ignore: bool,

    /// Output format
    #[arg(short, long, value_enum, default_value_t = Format::Text)]
    output: Format,

    /// Attribute each language's lines to source or tests [default]
    #[arg(long, overrides_with = "no_attribution")]
    attribution: bool,

    /// Count each language as one row, without telling source from tests, which is faster
    #[arg(long, overrides_with = "attribution")]
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

    fn json(&self) -> Value {
        let LineCounts { blank, comment, code } = self.lines;
        let lines = blank + comment + code;
        json!({ "files": self.files, "lines": lines, "blank": blank, "comment": comment, "code": code })
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

    // A path inside another is already counted with it.
    let mut paths = Vec::new();
    for path in &args.paths {
        let path =
            std::fs::canonicalize(path).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
        paths.push(path);
    }
    paths.sort();
    paths.dedup();
    let paths: Vec<PathBuf> = paths
        .iter()
        .filter(|p| !paths.iter().any(|q| q != *p && p.starts_with(q)))
        .cloned()
        .collect();
    let types: Vec<_> = paths.iter().map(|p| std::fs::metadata(p).map(|m| m.file_type())).collect();
    let is_dir = |i: usize| types[i].as_ref().is_ok_and(|t| t.is_dir());
    let roots = paths
        .iter()
        .enumerate()
        .map(|(i, p)| if is_dir(i) { p.clone() } else { p.parent().unwrap_or(p).to_path_buf() });
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
                    entry["language"] = json!(name);
                    entry["kind"] = json!(match kind {
                        Kind::Source => "source",
                        Kind::Test => "test",
                        Kind::Code => "code",
                        Kind::Other => "other",
                    });
                    if row.packages.keys().any(|(_, name)| name.is_some()) {
                        let packages = row.packages.iter().map(|((group, name), stats)| {
                            let mut package = stats.json();
                            package["name"] = json!(name.as_deref());
                            if !group.is_empty() {
                                package["project"] = json!(group.as_ref());
                            }
                            package
                        });
                        entry["packages"] = packages.collect();
                    }
                    entry
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
            let color = use_color();
            let paint = |code: u8, text: &str| {
                if color { format!("\x1b[38;5;{code}m{text}\x1b[0m") } else { text.to_string() }
            };
            // Each line is (depth, label, stats, whether it breaks down the row above it).
            let mut lines = Vec::new();
            // Source is only called source when the language has tests to tell it from.
            let has_tests = |name: &str| rows.contains_key(&(name.to_string(), Kind::Test));
            for ((name, kind), row) in &sorted {
                let label = match kind {
                    Kind::Source if has_tests(name) => format!("{name} (source)"),
                    Kind::Test => format!("{name} (tests)"),
                    Kind::Source | Kind::Code | Kind::Other => name.clone(),
                };
                lines.push((0, label, row.stats, false));
                let parts = match kind {
                    Kind::Other => row.breakdown(other_depth, false),
                    Kind::Source | Kind::Test | Kind::Code => row.breakdown(depth, true),
                };
                lines.extend(parts.into_iter().map(|(d, l, s)| (d, l.to_string(), s, true)));
            }
            // The label column is wide enough for the longest label, and at least 25.
            let width =
                lines.iter().map(|(d, l, ..)| 2 * d + l.chars().count()).fold(25, usize::max);
            let rule = paint(245, &"\u{2500}".repeat(width + 47));
            let header = format!(
                "{:<width$} {:>5}  {:>8}  {:>8}  {:>8}  {:>8}",
                "Language", "files", "lines", "blank", "comment", "code"
            );
            println!("{rule}\n{}\n{rule}", paint(74, &header));
            for (depth, label, stats, is_part) in &lines {
                let indent = "  ".repeat(*depth);
                let line = format!("{indent}{}", stats.text(label, width - indent.len()));
                println!("{}", if *is_part { paint(250, &line) } else { line });
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
        name != "node_modules"
            && !context.language.excludes(&name)
            && !context.root_language.excludes(&name)
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
            .then(code[b.as_str()].cmp(&code[a.as_str()]))
            .then(a.cmp(b))
            .then(a_kind.cmp(b_kind))
    });
    sorted
}

/// Count one walked file into `rows`, if it is a file of a counted type and at most `max_bytes`.
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
    let key = cloc::file_key(path);
    if !cloc::is_counted(&key) {
        return;
    }
    let Some(content) = read(path, max_bytes, buffer) else { return };
    let Some(context) = path.parent().map(|dir| resolver.context(dir)) else { return };
    let from = |base: &Path| path.strip_prefix(base).unwrap_or(path);
    // A test by the walked root's conventions stays a test inside a nested project, such as a
    // fixture under `tests/`.
    let is_test = || {
        Classifier::of(context.root_language).is_test(from(&context.root))
            || Classifier::of(context.language).is_test(from(&context.root))
            || Classifier::of(context.language).is_test(from(&context.project))
    };
    let lines = cloc::count_lines(content, &key);

    // A file is counted once, in the row that its path decides. Only its lines are divided: the
    // lines of a Rust source file's inline test modules count as tests.
    let kind = |is_test: bool| if is_test { Kind::Test } else { Kind::Source };
    let parts = if !cloc::is_split(&key) {
        vec![(Kind::Other, 1, lines)]
    } else if !attribute {
        vec![(Kind::Code, 1, lines)]
    } else if key == "rs" && !is_test() {
        let mut test = LineCounts::default();
        for span in cfg_test::test_spans(content) {
            if let Some(module) = content.get(span) {
                test.add(cloc::count_lines(module, &key));
            }
        }
        vec![(Kind::Source, 1, lines.without(test)), (Kind::Test, 0, test)]
    } else {
        vec![(kind(is_test()), 1, lines)]
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
/// that size, so a file that grows while it is read is cut at the size it had when opened.
fn read<'a>(path: &Path, max_bytes: u64, buffer: &'a mut Vec<u8>) -> Option<&'a [u8]> {
    let file = File::open(path).ok()?;
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
    buffer.reserve_exact(usize::try_from(len).ok()?);
    // Reading through `take` stops at `len` without another read call to find the end.
    file.take(len).read_to_end(buffer).ok()?;
    Some(buffer)
}

/// The most bytes that a reused read buffer keeps.
const LARGE_BUFFER: usize = 16 * 1024 * 1024;

/// Whether to color stdout.
///
/// `NO_COLOR` disables color and `COLOR` forces it, whatever their values. Otherwise color is
/// used only on a terminal outside an agent or CI environment.
fn use_color() -> bool {
    let set = |name: &str| std::env::var_os(name).is_some();
    if set("NO_COLOR") {
        return false;
    }
    set("COLOR")
        || (std::io::stdout().is_terminal()
            && !["CLAUDE_CODE", "CODEX", "CURSOR", "CI"].into_iter().any(set))
}
