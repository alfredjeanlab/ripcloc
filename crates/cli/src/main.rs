// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! `ripcloc`, which counts lines of code by language, split into source and tests.

mod cfg_test;
mod cloc;
mod lang;
mod packages;
mod project;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use clap::{Parser, ValueEnum};
use ignore::{DirEntry, WalkBuilder, WalkState};
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

    /// Levels of breakdown under each language: top-level directories, then the packages in them
    /// [default: 1, or 0 for documentation and configuration formats when there are many
    /// languages]
    #[arg(long)]
    depth: Option<usize>,

    /// Skip files larger than this many megabytes
    #[arg(long, value_name = "MB", default_value_t = 10)]
    max_file_size: u64,

    /// Output format
    #[arg(short, long, value_enum, default_value_t = Format::Text)]
    output: Format,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    Text,
    Json,
}

/// The label of the files outside every package.
const REST: &str = "\u{2026}";

/// Whether a row counts source, tests, or a format that is not split into either.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Kind {
    Source,
    Test,
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
        self.lines.blank += other.lines.blank;
        self.lines.comment += other.lines.comment;
        self.lines.code += other.lines.code;
    }

    fn json(&self) -> Value {
        let LineCounts { blank, comment, code } = self.lines;
        json!({ "files": self.files, "blank": blank, "comment": comment, "code": code })
    }

    fn text(&self, label: &str, width: usize) -> String {
        let LineCounts { blank, comment, code } = self.lines;
        format!("{label:<width$} {:>5}  {blank:>8}  {comment:>8}  {code:>8}", self.files)
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
    let roots = paths
        .iter()
        .map(|p| if p.is_dir() { p.clone() } else { p.parent().unwrap_or(p).to_path_buf() });
    let resolver = Arc::new(Resolver::new(roots));

    let Some((first, rest)) = paths.split_first() else { return Ok(()) };
    let mut builder = WalkBuilder::new(first);
    for path in rest {
        builder.add(path);
    }
    let filter_resolver = Arc::clone(&resolver);
    builder.max_depth(Some(args.max_depth)).follow_links(true).filter_entry(move |e| {
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

    let rows: Mutex<HashMap<(String, Kind), Row>> = Mutex::default();
    builder.build_parallel().run(|| {
        let (resolver, rows) = (&resolver, &rows);
        let max_bytes = args.max_file_size.saturating_mul(1024 * 1024);
        Box::new(move |entry| {
            if let Ok(entry) = entry {
                count_file(&entry, max_bytes, resolver, rows);
            }
            WalkState::Continue
        })
    });
    let rows = rows.into_inner().unwrap_or_else(PoisonError::into_inner);

    // Languages with the most code come first, each with its source before its tests.
    let sorted = sort_rows(&rows);
    let (mut source, mut test, mut other) = (Stats::default(), Stats::default(), Stats::default());
    for ((_, kind), row) in &sorted {
        match kind {
            Kind::Source => &mut source,
            Kind::Test => &mut test,
            Kind::Other => &mut other,
        }
        .add(row.stats);
    }
    let mut total = source;
    total.add(test);
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
            let totals = json!({ "source": source.json(), "test": test.json(), "other": other.json(), "total": total.json() });
            let report = json!({ "languages": languages, "totals": totals });
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
                    Kind::Source | Kind::Other => name.clone(),
                };
                lines.push((0, label, row.stats, false));
                let parts = match kind {
                    Kind::Other => row.breakdown(other_depth, false),
                    Kind::Source | Kind::Test => row.breakdown(depth, true),
                };
                lines.extend(parts.into_iter().map(|(d, l, s)| (d, l.to_string(), s, true)));
            }
            // The label column is wide enough for the longest label, and at least 25.
            let width =
                lines.iter().map(|(d, l, ..)| 2 * d + l.chars().count()).fold(25, usize::max);
            let rule = paint(245, &"\u{2500}".repeat(width + 37));
            let header = format!(
                "{:<width$} {:>5}  {:>8}  {:>8}  {:>8}",
                "Language", "files", "blank", "comment", "code"
            );
            println!("{rule}\n{}\n{rule}", paint(74, &header));
            for (depth, label, stats, is_part) in &lines {
                let indent = "  ".repeat(*depth);
                let line = format!("{indent}{}", stats.text(label, width - indent.len()));
                println!("{}", if *is_part { paint(250, &line) } else { line });
            }
            println!(
                "{rule}\n{}\n{}",
                source.text("Source total", width),
                test.text("Test total", width)
            );
            if other.files > 0 {
                println!("{}", other.text("Other total", width));
            }
            println!("{rule}\n{}\n{rule}", total.text("Total", width));
        }
    }
    Ok(())
}

/// The rows in report order: languages split into source and tests before the formats that are
/// not, each with the most code first, and a language's source before its tests.
fn sort_rows(rows: &HashMap<(String, Kind), Row>) -> Vec<(&(String, Kind), &Row)> {
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
fn count_file(
    entry: &DirEntry,
    max_bytes: u64,
    resolver: &Resolver,
    rows: &Mutex<HashMap<(String, Kind), Row>>,
) {
    let path = entry.path();
    let key = cloc::file_key(path);
    if !cloc::is_counted(&key)
        || !entry.file_type().is_some_and(|t| t.is_file())
        || entry.metadata().map_or(true, |m| m.len() > max_bytes)
    {
        return;
    }
    let Ok(content) = std::fs::read_to_string(path) else { return };
    let Some(context) = path.parent().map(|dir| resolver.context(dir)) else { return };
    let from = |base: &Path| path.strip_prefix(base).unwrap_or(path);
    // A test by the walked root's conventions stays a test inside a nested project, such as a
    // fixture under `tests/`.
    let is_test = Classifier::of(context.root_language).is_test(from(&context.root))
        || Classifier::of(context.language).is_test(from(&context.root))
        || Classifier::of(context.language).is_test(from(&context.project));
    let lines = cloc::count_lines(&content, &key);

    // A Rust source file's inline test modules count as tests, so its counts are split in
    // proportion to its source and test lines.
    let kind = |is_test: bool| if is_test { Kind::Test } else { Kind::Source };
    let parts = if !cloc::is_split(&key) {
        vec![(Kind::Other, 1, lines)]
    } else if key == "rs" && !is_test {
        let (source, test) = cfg_test::split_lines(&content);
        let nonblank = (lines.comment + lines.code).max(1) as f64;
        let share = |n: usize, part: usize| (n as f64 * part as f64 / nonblank).round() as usize;
        let part = |count: usize| LineCounts {
            blank: share(lines.blank, count),
            comment: share(lines.comment, count),
            code: share(lines.code, count),
        };
        vec![(Kind::Source, source, part(source)), (Kind::Test, test, part(test))]
    } else {
        vec![(kind(is_test), 1, lines)]
    };

    let language_name = cloc::language_name(&key);
    let mut rows = rows.lock().unwrap_or_else(PoisonError::into_inner);
    for (kind, _, lines) in parts.into_iter().filter(|(_, count, _)| *count > 0) {
        let stats = Stats { files: 1, lines };
        let row = rows.entry((language_name.to_string(), kind)).or_default();
        row.stats.add(stats);
        let key = (Arc::clone(&context.group), context.package.clone());
        row.packages.entry(key).or_default().add(stats);
        row.chains.entry(Arc::clone(&context.chain)).or_default().add(stats);
    }
}

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
