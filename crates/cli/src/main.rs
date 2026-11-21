// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! `ripcloc`, which counts lines of code very fast.

mod args;
mod cfg_test;
mod color;
mod lang;
mod notebook;
mod packages;
mod project;
mod syntax;

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use ignore::{WalkBuilder, WalkState};
use serde_json::{Map, Value, json};

use args::{Args, Format};
use lang::{Classifier, DirKind, Language};
use project::Resolver;
use syntax::LineCounts;

/// The version of the JSON report's shape. It changes only when a field is removed or changes
/// meaning, never when counts change or a field is added.
const SCHEMA: u32 = 1;

/// The label of the files outside every package, and of the small rows folded into them.
const REST: &str = "\u{2026}";

/// The smallest share of its parent's lines, in percent, that a breakdown row has without a
/// `--depth`. A smaller row is folded into [`REST`].
const MIN_SHARE_PERCENT: usize = 2;

/// The smallest share of the report's lines, in percent, that a row has to be broken down without
/// a `--depth`. A smaller language is one row.
const MIN_LANGUAGE_PERCENT: usize = 1;

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

    fn line_count(&self) -> usize {
        let LineCounts { blank, comment, code } = self.lines;
        blank + comment + code
    }

    fn json(&self) -> Map<String, Value> {
        let LineCounts { blank, comment, code } = self.lines;
        let lines = self.line_count();
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
        let lines = self.line_count();
        // A row with lines but no files holds the inline tests of files that count as source, so
        // its file count does not apply rather than being zero.
        let files =
            if self.files == 0 && lines > 0 { "–".to_string() } else { self.files.to_string() };
        format!("{label:<width$} {files:>5}  {lines:>8}  {blank:>8}  {comment:>8}  {code:>8}")
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
    /// row's files are. With `fold`, a row with less than [`MIN_SHARE_PERCENT`] of the lines of
    /// the row above it is folded into [`REST`].
    fn breakdown(&self, depth: usize, single: bool, fold: bool) -> Vec<(usize, &str, Stats)> {
        let mut lines = Vec::new();
        self.level(&[], depth, single, fold, &mut lines);
        lines
    }

    fn level<'a>(
        &'a self,
        prefix: &[&'a str],
        depth: usize,
        single: bool,
        fold: bool,
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
        if fold {
            let parent =
                rest.line_count() + children.values().map(Stats::line_count).sum::<usize>();
            children.retain(|_, stats| {
                let keep = stats.line_count() * 100 >= parent * MIN_SHARE_PERCENT;
                if !keep {
                    rest.add(*stats);
                }
                keep
            });
        }
        // The lines of inline tests make a row without files.
        let has_rest = rest.files > 0 || rest.line_count() > 0;
        if children.is_empty() || (!single && children.len() + usize::from(has_rest) < 2) {
            return;
        }
        for (label, stats) in children {
            lines.push((prefix.len() + 1, label, stats));
            let mut next = prefix.to_vec();
            next.push(label);
            self.level(&next, depth, single, fold, lines);
        }
        if has_rest {
            lines.push((prefix.len() + 1, REST, rest));
        }
    }
}

/// The report of every language and format that is counted, with its extensions and file names,
/// and whether its files are split into source and tests or counted as other.
///
/// The text has a row for each, sorted by name, with its extensions and then, after `named`, its
/// file names. An extension that the content decides is marked `(by content)`. A language whose
/// files are counted in two ways, such as Terraform's code and configuration, has a row for each,
/// and its name only on the first.
fn languages_report(format: Format) -> anyhow::Result<String> {
    let mut languages: Vec<_> = syntax::LANGUAGES.iter().collect();
    languages.sort_by_key(|l| (l.name.to_lowercase(), !l.split));
    let kind = |split: bool| if split { "source and tests" } else { "other" };
    let mut out = String::new();
    match format {
        Format::Json => {
            let languages: Vec<Value> = languages
                .iter()
                .map(|l| {
                    json!({
                        "language": l.name,
                        "kind": if l.split { "split" } else { "other" },
                        "extensions": l.extensions,
                        "extensions_by_content": l.shared,
                        "names": l.names,
                    })
                })
                .collect();
            let report = json!({ "schema": SCHEMA, "languages": languages });
            writeln!(out, "{}", serde_json::to_string_pretty(&report)?)?;
        }
        Format::Text => {
            let width = languages.iter().map(|l| l.name.chars().count()).fold(8, usize::max);
            let kind_width = kind(true).len();
            writeln!(
                out,
                "{:<width$}  {:<kind_width$}  Extensions and file names",
                "Language", "Counted as"
            )?;
            let mut previous = None;
            for l in languages {
                let extensions = l.extensions.iter().map(|e| format!(".{e}"));
                let shared = l.shared.iter().map(|e| format!(".{e} (by content)"));
                let names = (!l.names.is_empty()).then(|| format!("named {}", l.names.join(" ")));
                let files: Vec<_> = extensions.chain(shared).chain(names).collect();
                let name = if previous == Some(l.name) { "" } else { l.name };
                previous = Some(l.name);
                let row =
                    format!("{name:<width$}  {:<kind_width$}  {}", kind(l.split), files.join(" "));
                writeln!(out, "{}", row.trim_end())?;
            }
        }
    }
    Ok(out)
}

fn main() -> anyhow::Result<()> {
    let mut args = Args::parse();
    // The listing of `--languages` belongs here, before any path is read.
    if args.languages {
        return write_report(&languages_report(args.output)?);
    }
    if args.compact {
        args.no_attribution = true;
        args.depth = args.depth.or(Some(0));
    }

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
    let (rows, unreadable) = if paths.len() <= DIRECT_FILES
        && types.iter().all(|t| t.as_ref().is_ok_and(|t| t.is_file()))
    {
        let (mut rows, mut unreadable, mut buffer) = (Rows::new(), 0, Vec::new());
        for path in &paths {
            let attribute = !args.no_attribution;
            count_file(
                path,
                max_bytes,
                attribute,
                &resolver,
                &mut rows,
                &mut unreadable,
                &mut buffer,
            );
        }
        (rows, unreadable)
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

    // The report is written at once, so that a reader that closes early is handled in one place.
    let mut out = String::new();
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
            writeln!(out, "{}", serde_json::to_string_pretty(&report)?)?;
        }
        // Unreadable files are skipped without a warning, since warnings would bury the report,
        // so they are mentioned only when there is no report for a warning to bury.
        Format::Text if sorted.is_empty() => match unreadable {
            0 => writeln!(out, "No source files found.")?,
            1 => writeln!(out, "No source files found (skipped 1 unreadable file).")?,
            n => writeln!(out, "No source files found (skipped {n} unreadable files).")?,
        },
        Format::Text => {
            // Without a `--depth`, the formats that are not split into source and tests are one
            // number each, small rows are folded together, a breakdown of one row is left out
            // because it repeats the row above it, and a language with little of the report's code
            // is not broken down.
            let other_depth = args.depth.unwrap_or(0);
            let depth = args.depth.unwrap_or(1);
            let fold = args.depth.is_none();
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
                let small =
                    row.stats.line_count() * 100 < total.line_count() * MIN_LANGUAGE_PERCENT;
                let parts = match kind {
                    _ if fold && small => Vec::new(),
                    Kind::Other => row.breakdown(other_depth, false, fold),
                    Kind::Source | Kind::Test | Kind::Code => row.breakdown(depth, !fold, fold),
                };
                lines.extend(parts.into_iter().map(|(d, l, s)| (d, l.to_string(), s)));
            }
            // The label column is wide enough for the longest label, and at least 25.
            let width =
                lines.iter().map(|(d, l, ..)| 2 * d + l.chars().count()).fold(25, usize::max);
            let on = color::enabled();
            let rule = color::paint(on, color::CONTEXT, &"\u{2500}".repeat(width + 47));
            let header = format!(
                "{:<width$} {:>5}  {:>8}  {:>8}  {:>8}  {:>8}",
                "Language", "files", "lines", "blank", "comment", "code"
            );
            writeln!(out, "{rule}\n{}\n{rule}", color::paint(on, color::HEADER, &header))?;
            for (depth, label, stats) in &lines {
                let indent = "  ".repeat(*depth);
                let line = format!("{indent}{}", stats.text(label, width - indent.len()));
                // The rows that break down the row above them are lighter than it.
                let line = if *depth > 0 { color::paint(on, color::LITERAL, &line) } else { line };
                writeln!(out, "{line}")?;
            }
            // Without attribution, code and other formats are only totaled apart when there are both.
            if !args.no_attribution {
                writeln!(
                    out,
                    "{rule}\n{}\n{}",
                    source.text("Total source", width),
                    test.text("Total tests", width)
                )?;
                if other.files > 0 {
                    writeln!(out, "{}", other.text("Total other", width))?;
                }
            } else if code.files > 0 && other.files > 0 {
                writeln!(
                    out,
                    "{rule}\n{}\n{}",
                    code.text("Total code", width),
                    other.text("Total other", width)
                )?;
            }
            writeln!(out, "{rule}\n{}\n{rule}", total.text("Total", width))?;
        }
    }
    write_report(&out)
}

/// Write `report` to standard output at once, so that a reader that closes early is handled in
/// one place.
fn write_report(report: &str) -> anyhow::Result<()> {
    let mut stdout = std::io::stdout().lock();
    match stdout.write_all(report.as_bytes()).and_then(|()| stdout.flush()) {
        // A reader that stops early, such as `head`, has read all that it wanted.
        Err(e) if e.kind() == ErrorKind::BrokenPipe => Ok(()),
        written => Ok(written?),
    }
}

/// The most paths that are counted without the walker when every one is a file.
const DIRECT_FILES: usize = 8;

/// Count the files under `paths` on the walker's threads, and return the rows with the number of
/// files that could not be read.
fn walk(paths: &[PathBuf], args: &Args, resolver: &Arc<Resolver>, max_bytes: u64) -> (Rows, usize) {
    let Some((first, rest)) = paths.split_first() else { return (Rows::new(), 0) };
    let mut builder = WalkBuilder::new(first);
    for path in rest {
        builder.add(path);
    }
    let filter_resolver = Arc::clone(resolver);
    let deps = args.deps;
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

    // Each thread counts into rows of its own, which are merged when the walk drops it.
    let rows = Mutex::new(Rows::new());
    let unreadable = AtomicUsize::new(0);
    builder.build_parallel().run(|| {
        let mut local = ThreadRows {
            rows: Rows::new(),
            unreadable: 0,
            buffer: Vec::new(),
            merged: &rows,
            merged_unreadable: &unreadable,
        };
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
                    &mut local.unreadable,
                    &mut local.buffer,
                );
            }
            WalkState::Continue
        })
    });
    (rows.into_inner().unwrap_or_else(PoisonError::into_inner), unreadable.into_inner())
}

/// The rows and the number of unreadable files that one walker thread counts into, and a buffer
/// that it reads files into.
struct ThreadRows<'a> {
    rows: Rows,
    unreadable: usize,
    buffer: Vec<u8>,
    merged: &'a Mutex<Rows>,
    merged_unreadable: &'a AtomicUsize,
}

impl Drop for ThreadRows<'_> {
    fn drop(&mut self) {
        let mut merged = self.merged.lock().unwrap_or_else(PoisonError::into_inner);
        for (key, row) in std::mem::take(&mut self.rows) {
            merged.entry(key).or_default().merge(row);
        }
        self.merged_unreadable.fetch_add(self.unreadable, Ordering::Relaxed);
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
/// `max_bytes`, or add one to `unreadable` if the file cannot be read.
///
/// Without `attribute`, the file's language is one row, and no time is spent on telling tests apart.
fn count_file(
    path: &Path,
    max_bytes: u64,
    attribute: bool,
    resolver: &Resolver,
    rows: &mut Rows,
    unreadable: &mut usize,
    buffer: &mut Vec<u8>,
) {
    let mut key = syntax::file_key(path);
    let script = key.is_empty();
    if script && is_never_script(path) || !script && !syntax::is_counted(&key) {
        return;
    }
    let content = match read(path, max_bytes, script, buffer) {
        Ok(Some(content)) => content,
        Ok(None) => return,
        Err(_) => {
            *unreadable += 1;
            return;
        }
    };
    if script {
        let start = content.get(..SHEBANG_PREFIX).unwrap_or(content);
        let Some(shebang) = syntax::shebang_key(start).filter(|k| syntax::is_counted(k)) else {
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
    // Coq and Verilog share the extension `.v`, Perl and Prolog share `.pl`, and D shares `.d`
    // with the lists of dependencies that compilers write, so the content tells them apart. A
    // notebook is counted here, since one that is not valid JSON is not counted at all.
    let mut notebook = None;
    match key.as_str() {
        "v" => key = syntax::v_key(content).to_string(),
        "pl" => key = syntax::pl_key(content).to_string(),
        "d" if syntax::is_make_dependencies(content) => return,
        "ipynb" => {
            let Some(counts) = notebook::count(content) else { return };
            notebook = Some(counts);
        }
        _ => {}
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
    let lines = || notebook.unwrap_or_else(|| syntax::count_lines(content, &key));
    let parts = if !syntax::is_split(&key) {
        vec![(Kind::Other, 1, lines())]
    } else if !attribute {
        vec![(Kind::Code, 1, lines())]
    } else if key == "rs" && !is_test() {
        let (lines, test) = cfg_test::count(content);
        vec![(Kind::Source, 1, lines.without(test)), (Kind::Test, 0, test)]
    } else {
        vec![(kind(is_test()), 1, lines())]
    };

    let language_name = syntax::language_name(&key);
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
