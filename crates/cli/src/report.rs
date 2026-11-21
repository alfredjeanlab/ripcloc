// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! The reports of the counts and of the counted languages, as text or JSON.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use ripcloc::syntax::{self, LineCounts};
use ripcloc::{Counts, Kind, Row, Rows, Stats};
use serde_json::{Map, Value, json};

use crate::args::{Args, Format};
use crate::color;

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

/// A row of the report with its language name and kind.
type Entry<'a> = (&'a (String, Kind), &'a Row);

/// The report of `counts`, in the format and with the breakdowns that `args` asks for.
pub(crate) fn counts(counts: &Counts, args: &Args) -> anyhow::Result<String> {
    let sorted = sort_rows(&counts.rows);
    let totals = Totals::of(&sorted);
    match args.output {
        Format::Json => json_report(&sorted, &totals, !args.no_attribution),
        // Unreadable files are skipped without a warning, since warnings would bury the report,
        // so they are mentioned only when there is no report for a warning to bury.
        Format::Text if sorted.is_empty() => Ok(match counts.unreadable {
            0 => "No source files found.\n".to_string(),
            1 => "No source files found (skipped 1 unreadable file).\n".to_string(),
            n => format!("No source files found (skipped {n} unreadable files).\n"),
        }),
        Format::Text => text_report(&sorted, &counts.rows, &totals, args),
    }
}

/// The rows in report order. Languages that are split into source and tests come before the
/// formats that are not, each with the most code first, and a language's source comes before its
/// tests.
fn sort_rows(rows: &Rows) -> Vec<Entry<'_>> {
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

/// The sums of the rows of each kind, and of every row.
#[derive(Default)]
struct Totals {
    source: Stats,
    test: Stats,
    code: Stats,
    other: Stats,
    total: Stats,
}

impl Totals {
    fn of(rows: &[Entry<'_>]) -> Totals {
        let mut totals = Totals::default();
        for ((_, kind), row) in rows {
            match kind {
                Kind::Source => &mut totals.source,
                Kind::Test => &mut totals.test,
                Kind::Code => &mut totals.code,
                Kind::Other => &mut totals.other,
            }
            .add(row.stats);
            totals.total.add(row.stats);
        }
        totals
    }
}

fn json_report(sorted: &[Entry<'_>], totals: &Totals, attribution: bool) -> anyhow::Result<String> {
    let languages: Vec<Value> =
        sorted.iter().map(|((name, kind), row)| json_row(name, *kind, row)).collect();
    let Totals { source, test, code, other, total } = totals;
    let totals = if attribution {
        json!({
            "source": json_stats(source),
            "test": json_stats(test),
            "other": json_stats(other),
            "total": json_stats(total),
        })
    } else {
        json!({ "code": json_stats(code), "other": json_stats(other), "total": json_stats(total) })
    };
    let report = json!({
        "schema": SCHEMA,
        "attribution": attribution,
        "languages": languages,
        "totals": totals,
    });
    Ok(format!("{}\n", serde_json::to_string_pretty(&report)?))
}

/// The JSON of one row, with its packages when any file is in a named package.
fn json_row(name: &str, kind: Kind, row: &Row) -> Value {
    let mut entry = json_stats(&row.stats);
    entry.insert("language".into(), json!(name));
    let kind = match kind {
        Kind::Source => "source",
        Kind::Test => "test",
        Kind::Code => "code",
        Kind::Other => "other",
    };
    entry.insert("kind".into(), json!(kind));
    if row.packages.keys().any(|(_, name)| name.is_some()) {
        let packages = row.packages.iter().map(|((group, name), stats)| {
            let mut package = json_stats(stats);
            package.insert("name".into(), json!(name.as_deref()));
            if !group.is_empty() {
                package.insert("project".into(), json!(group.as_ref()));
            }
            package
        });
        entry.insert("packages".into(), packages.collect());
    }
    Value::Object(entry)
}

fn json_stats(stats: &Stats) -> Map<String, Value> {
    let LineCounts { blank, comment, code } = stats.lines;
    let fields = [
        ("files", stats.files),
        ("lines", stats.line_count()),
        ("blank", blank),
        ("comment", comment),
        ("code", code),
    ];
    fields.into_iter().map(|(key, count)| (key.to_string(), json!(count))).collect()
}

fn text_report(
    sorted: &[Entry<'_>],
    rows: &Rows,
    totals: &Totals,
    args: &Args,
) -> anyhow::Result<String> {
    let lines = text_lines(sorted, rows, &totals.total, args.depth);
    // The label column is wide enough for the longest label, and at least 25.
    let width = lines.iter().map(|(d, l, ..)| 2 * d + l.chars().count()).fold(25, usize::max);
    let on = color::enabled();
    let rule = color::paint(on, color::CONTEXT, &"\u{2500}".repeat(width + 47));
    let header = format!(
        "{:<width$} {:>5}  {:>8}  {:>8}  {:>8}  {:>8}",
        "Language", "files", "lines", "blank", "comment", "code"
    );
    let mut out = String::new();
    writeln!(out, "{rule}\n{}\n{rule}", color::paint(on, color::HEADER, &header))?;
    for (depth, label, stats) in &lines {
        let indent = "  ".repeat(*depth);
        let line = format!("{indent}{}", text_stats(stats, label, width - indent.len()));
        // The rows that break down the row above them are lighter than it.
        let line = if *depth > 0 { color::paint(on, color::LITERAL, &line) } else { line };
        writeln!(out, "{line}")?;
    }
    let Totals { source, test, code, other, total } = totals;
    // Without attribution, code and other formats are only totaled apart when there are both.
    if !args.no_attribution {
        writeln!(out, "{rule}")?;
        writeln!(out, "{}", text_stats(source, "Total source", width))?;
        writeln!(out, "{}", text_stats(test, "Total tests", width))?;
        if other.files > 0 {
            writeln!(out, "{}", text_stats(other, "Total other", width))?;
        }
    } else if code.files > 0 && other.files > 0 {
        writeln!(out, "{rule}")?;
        writeln!(out, "{}", text_stats(code, "Total code", width))?;
        writeln!(out, "{}", text_stats(other, "Total other", width))?;
    }
    writeln!(out, "{rule}\n{}\n{rule}", text_stats(total, "Total", width))?;
    Ok(out)
}

/// The lines of the text report above its totals, as (depth, label, stats), with each row of
/// `sorted` followed by its breakdown.
///
/// Without a `depth`, the formats that are not split into source and tests are one number each,
/// small rows are folded together, a breakdown of one row is left out because it repeats the row
/// above it, and a language with little of the `total` lines is not broken down.
fn text_lines(
    sorted: &[Entry<'_>],
    rows: &Rows,
    total: &Stats,
    depth: Option<usize>,
) -> Vec<(usize, String, Stats)> {
    let fold = depth.is_none();
    let split = Breakdown { depth: depth.unwrap_or(1), single: !fold, fold };
    let other = Breakdown { depth: depth.unwrap_or(0), single: false, fold };
    // Source is only called source when the language has tests to tell it from.
    let has_tests = |name: &str| rows.contains_key(&(name.to_string(), Kind::Test));
    let mut lines = Vec::new();
    for ((name, kind), row) in sorted {
        let label = match kind {
            Kind::Source if has_tests(name) => format!("{name} (source)"),
            Kind::Test => format!("{name} (tests)"),
            Kind::Source | Kind::Code | Kind::Other => name.clone(),
        };
        lines.push((0, label, row.stats));
        let small = row.stats.line_count() * 100 < total.line_count() * MIN_LANGUAGE_PERCENT;
        let parts = match kind {
            _ if fold && small => Vec::new(),
            Kind::Other => other.rows(row),
            Kind::Source | Kind::Test | Kind::Code => split.rows(row),
        };
        lines.extend(parts.into_iter().map(|(d, l, s)| (d, l.to_string(), s)));
    }
    lines
}

fn text_stats(stats: &Stats, label: &str, width: usize) -> String {
    let LineCounts { blank, comment, code } = stats.lines;
    let lines = stats.line_count();
    // A row with lines but no files holds the inline tests of files that count as source, so
    // its file count does not apply rather than being zero.
    let files =
        if stats.files == 0 && lines > 0 { "–".to_string() } else { stats.files.to_string() };
    format!("{label:<width$} {files:>5}  {lines:>8}  {blank:>8}  {comment:>8}  {code:>8}")
}

/// How a row is broken down into the indented rows under it.
///
/// Each level lists the labels below the row above it, then [`REST`] for the files that are in
/// none of them, so that the rows at each level add up to the row above. A list of one row
/// repeats the row above it, so it is left out unless `single` keeps it to show where the row's
/// files are. With `fold`, a row with less than [`MIN_SHARE_PERCENT`] of the lines of the row
/// above it is folded into [`REST`].
struct Breakdown {
    depth: usize,
    single: bool,
    fold: bool,
}

impl Breakdown {
    /// The indented rows under `row`, as (depth, label, stats).
    fn rows<'a>(&self, row: &'a Row) -> Vec<(usize, &'a str, Stats)> {
        let mut lines = Vec::new();
        self.level(row, &[], &mut lines);
        lines
    }

    /// Add the rows under the chain of labels `prefix` of `row` to `lines`.
    fn level<'a>(
        &self,
        row: &'a Row,
        prefix: &[&'a str],
        lines: &mut Vec<(usize, &'a str, Stats)>,
    ) {
        if prefix.len() >= self.depth {
            return;
        }
        let mut children: BTreeMap<&str, Stats> = BTreeMap::new();
        let mut rest = Stats::default();
        for (chain, stats) in &row.chains {
            let labels: Vec<&str> = chain.iter().map(|l| &**l).collect();
            if !labels.starts_with(prefix) {
                continue;
            }
            match labels.get(prefix.len()) {
                Some(label) => children.entry(label).or_default().add(*stats),
                None => rest.add(*stats),
            }
        }
        if self.fold {
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
        if children.is_empty() || (!self.single && children.len() + usize::from(has_rest) < 2) {
            return;
        }
        for (label, stats) in children {
            lines.push((prefix.len() + 1, label, stats));
            let mut next = prefix.to_vec();
            next.push(label);
            self.level(row, &next, lines);
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
pub(crate) fn languages(format: Format) -> anyhow::Result<String> {
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
