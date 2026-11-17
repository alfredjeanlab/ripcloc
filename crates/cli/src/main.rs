// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! `ripcloc`, which counts lines of code by language, split into source and tests.

mod cfg_test;
mod cloc;
mod lang;
mod packages;
mod project;

use std::collections::{BTreeMap, HashMap};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use clap::{Parser, ValueEnum};
use ignore::{DirEntry, WalkBuilder, WalkState};
use serde_json::{Value, json};

use cloc::LineCounts;
use lang::Classifier;
use project::Resolver;

/// Files larger than this are not counted.
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

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

    /// Output format
    #[arg(short, long, value_enum, default_value_t = Format::Text)]
    output: Format,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    Text,
    Json,
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

/// The counts of one language's source or tests, with the share of each package.
#[derive(Default)]
struct Row {
    stats: Stats,
    packages: BTreeMap<String, Stats>,
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

    let rows: Mutex<HashMap<(String, bool), Row>> = Mutex::default();
    builder.build_parallel().run(|| {
        let (resolver, rows) = (&resolver, &rows);
        Box::new(move |entry| {
            if let Ok(entry) = entry {
                count_file(&entry, resolver, rows);
            }
            WalkState::Continue
        })
    });
    let rows = rows.into_inner().unwrap_or_else(PoisonError::into_inner);

    // Languages with the most code come first, each with its source before its tests.
    let mut language_code: HashMap<&str, usize> = HashMap::new();
    for ((name, _), row) in &rows {
        *language_code.entry(name).or_default() += row.stats.lines.code;
    }
    let mut sorted: Vec<_> = rows.iter().collect();
    sorted.sort_by(|((a, a_test), _), ((b, b_test), _)| {
        language_code[b.as_str()]
            .cmp(&language_code[a.as_str()])
            .then(a.cmp(b))
            .then(a_test.cmp(b_test))
    });
    let (mut source, mut test) = (Stats::default(), Stats::default());
    for ((_, is_test), row) in &sorted {
        if *is_test { &mut test } else { &mut source }.add(row.stats);
    }
    let mut total = source;
    total.add(test);

    match args.output {
        Format::Json => {
            let languages: Vec<Value> = sorted
                .iter()
                .map(|((name, is_test), row)| {
                    let mut entry = row.stats.json();
                    entry["language"] = json!(name);
                    entry["kind"] = json!(if *is_test { "test" } else { "source" });
                    if !row.packages.is_empty() {
                        let packages = row.packages.iter().map(|(name, stats)| {
                            let mut package = stats.json();
                            package["name"] = json!(name);
                            package
                        });
                        entry["packages"] = packages.collect();
                    }
                    entry
                })
                .collect();
            let totals =
                json!({ "source": source.json(), "test": test.json(), "total": total.json() });
            let report = json!({ "languages": languages, "totals": totals });
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Format::Text if sorted.is_empty() => println!("No source files found."),
        Format::Text => {
            let color = use_color();
            let paint = |code: u8, text: &str| {
                if color { format!("\x1b[38;5;{code}m{text}\x1b[0m") } else { text.to_string() }
            };
            let rule = paint(245, &"\u{2500}".repeat(62));
            let header = format!(
                "{:<25} {:>5}  {:>8}  {:>8}  {:>8}",
                "Language", "files", "blank", "comment", "code"
            );
            println!("{rule}\n{}\n{rule}", paint(74, &header));
            for ((name, is_test), row) in &sorted {
                let kind = if *is_test { "tests" } else { "source" };
                println!("{}", row.stats.text(&format!("{name} ({kind})"), 25));
                for (package, stats) in &row.packages {
                    println!("{}", paint(250, &format!("  {}", stats.text(package, 23))));
                }
            }
            println!(
                "{rule}\n{}\n{}",
                source.text("Source total", 25),
                test.text("Test total", 25)
            );
            println!("{rule}\n{}\n{rule}", total.text("Total", 25));
        }
    }
    Ok(())
}

/// Count one walked file into `rows`, if it is a file of a counted type.
fn count_file(entry: &DirEntry, resolver: &Resolver, rows: &Mutex<HashMap<(String, bool), Row>>) {
    let path = entry.path();
    let key = cloc::file_key(path);
    if !cloc::is_counted(&key)
        || !entry.file_type().is_some_and(|t| t.is_file())
        || entry.metadata().map_or(true, |m| m.len() > MAX_FILE_SIZE)
    {
        return;
    }
    let Ok(content) = std::fs::read_to_string(path) else { return };
    let Some(context) = path.parent().map(|dir| resolver.context(dir)) else { return };
    let from = |base: &Path| path.strip_prefix(base).unwrap_or(path);
    let classifier = Classifier::of(context.language);
    let is_test =
        classifier.is_test(from(&context.root)) || classifier.is_test(from(&context.project));
    let lines = cloc::count_lines(&content, &key);

    // A Rust source file's inline test modules count as tests, so its counts are split in
    // proportion to its source and test lines.
    let parts = if key == "rs" && !is_test {
        let (source, test) = cfg_test::split_lines(&content);
        let nonblank = (lines.comment + lines.code).max(1) as f64;
        let share = |n: usize, part: usize| (n as f64 * part as f64 / nonblank).round() as usize;
        let part = |count: usize| LineCounts {
            blank: share(lines.blank, count),
            comment: share(lines.comment, count),
            code: share(lines.code, count),
        };
        vec![(false, source, part(source)), (true, test, part(test))]
    } else {
        vec![(is_test, 1, lines)]
    };

    let language_name = cloc::language_name(&key);
    let mut rows = rows.lock().unwrap_or_else(PoisonError::into_inner);
    for (is_test, _, lines) in parts.into_iter().filter(|(_, count, _)| *count > 0) {
        let stats = Stats { files: 1, lines };
        let row = rows.entry((language_name.to_string(), is_test)).or_default();
        row.stats.add(stats);
        if let Some(package) = &context.package {
            row.packages.entry(package.to_string()).or_default().add(stats);
        }
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
