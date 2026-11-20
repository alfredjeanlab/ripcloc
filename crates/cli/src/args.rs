// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! The command line, which is parsed with `lexopt` and described by a help text written by hand.

use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

use lexopt::Arg::{Long, Short, Value};

/// The text of `-h` and `--help`. It leaves out `--max-depth` and `--max-file-size`, which still
/// work but are deprecated without a warning.
const HELP: &str = "\
Counts lines of code very fast.

Usage: ripcloc [OPTIONS] [PATH]...

Arguments:
  [PATH]...               The files and directories to count [default: .]

Options:
  -o, --output <FORMAT>   Print the report as text or json [default: text]
  -c, --compact           Count each language as one row, with no test split or breakdown
      --depth <N>         Break each language down this many levels, from top-level directories
                          to the packages of nested workspaces [default: auto]
      --[no-]attribution  Attribute each language's lines to source or tests. --no-attribution
                          counts each language as one row, which is faster [default: on]
      --[no-]ignore       Respect .gitignore, .ignore and git's exclude files [default: on]
      --[no-]large        Count files of any size, or with --no-large skip files over 10 MB, such
                          as generated code and data [default: skip files over 1 GiB]
      --[no-]deps         Count dependency directories, such as vendor/, node_modules/ and
                          Python virtual environments, unless ignored [default: off]
      --languages         Print the languages that ripcloc counts and exit
  -h, --help              Print this help and exit
  -V, --version           Print the version and exit
";

/// The text of `-V` and `--version`.
const VERSION: &str = concat!("ripcloc ", env!("CARGO_PKG_VERSION"), "\n");

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Format {
    Text,
    Json,
}

/// The options of one run. Each flag of a `--x` and `--no-x` pair has a field of its own, so that
/// a pair that is given together is an error instead of the last one winning.
pub(crate) struct Args {
    /// The files and directories to count, which are `.` when none is given.
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) output: Format,
    pub(crate) compact: bool,
    pub(crate) depth: Option<usize>,
    pub(crate) attribution: bool,
    pub(crate) no_attribution: bool,
    pub(crate) ignore: bool,
    pub(crate) no_ignore: bool,
    pub(crate) large: bool,
    pub(crate) no_large: bool,
    /// `--deps` counts dependency directories, and `--no-deps`, the default, skips them.
    pub(crate) deps: bool,
    pub(crate) no_deps: bool,
    /// `--languages` prints the languages that ripcloc counts instead of counting any path.
    pub(crate) languages: bool,
    /// The size in megabytes over which a file is skipped. It is left out of the help, since
    /// `--large` and `--no-large` replace it.
    pub(crate) max_file_size: Option<u64>,
    /// The deepest directory that is walked. It is left out of the help, since a limit other
    /// than the default is rarely useful.
    pub(crate) max_depth: usize,
}

/// What the command line asks for.
enum Parsed {
    Run(Args),
    Help,
    Version,
}

impl Args {
    /// Parse the arguments of the process. Help and the version are printed to stdout with exit
    /// status 0, and a usage error is printed to stderr with exit status 2.
    pub(crate) fn parse() -> Args {
        match parse(lexopt::Parser::from_env()) {
            Ok(Parsed::Run(args)) => args,
            Ok(Parsed::Help) => {
                let help =
                    if crate::color::enabled() { crate::color::help(HELP) } else { HELP.into() };
                print_and_exit(std::io::stdout(), &help, 0)
            }
            Ok(Parsed::Version) => print_and_exit(std::io::stdout(), VERSION, 0),
            Err(message) => {
                let text = format!("error: {message}\n\nFor more information, try '--help'.\n");
                print_and_exit(std::io::stderr(), &text, 2)
            }
        }
    }
}

/// Write `text` and exit with `code`. A reader that closes early, as `head` does, is not an
/// error.
fn print_and_exit(mut out: impl Write, text: &str, code: i32) -> ! {
    let _ = out.write_all(text.as_bytes()).and_then(|()| out.flush());
    std::process::exit(code)
}

fn parse(mut parser: lexopt::Parser) -> Result<Parsed, String> {
    let mut args = Args {
        paths: Vec::new(),
        output: Format::Text,
        compact: false,
        depth: None,
        attribution: false,
        no_attribution: false,
        ignore: false,
        no_ignore: false,
        large: false,
        no_large: false,
        deps: false,
        no_deps: false,
        languages: false,
        max_file_size: None,
        max_depth: 100,
    };
    while let Some(arg) = parser.next().map_err(describe)? {
        match arg {
            Short('o') | Long("output") => {
                let value = parser.value().map_err(describe)?;
                args.output = match value.to_str() {
                    Some("text") => Format::Text,
                    Some("json") => Format::Json,
                    _ => return Err(invalid(&value, "--output", "which is text or json")),
                };
            }
            Short('c') | Long("compact") => args.compact = true,
            Long("depth") => args.depth = Some(number(&mut parser, "--depth")?),
            Long("attribution") => args.attribution = true,
            Long("no-attribution") => args.no_attribution = true,
            Long("ignore") => args.ignore = true,
            Long("no-ignore") => args.no_ignore = true,
            Long("large") => args.large = true,
            Long("no-large") => args.no_large = true,
            Long("deps") => args.deps = true,
            Long("no-deps") => args.no_deps = true,
            Long("languages") => args.languages = true,
            Long("max-file-size") => {
                args.max_file_size = Some(number(&mut parser, "--max-file-size")?)
            }
            Long("max-depth") => args.max_depth = number(&mut parser, "--max-depth")?,
            Short('h') | Long("help") => return Ok(Parsed::Help),
            Short('V') | Long("version") => return Ok(Parsed::Version),
            Value(path) => args.paths.push(path.into()),
            Long(flag) => return Err(unknown(flag)),
            Short(flag) => return Err(format!("unknown flag -{flag}")),
        }
    }

    let conflicts = [
        (args.attribution, "--attribution", args.no_attribution, "--no-attribution"),
        (args.attribution, "--attribution", args.compact, "--compact"),
        (args.ignore, "--ignore", args.no_ignore, "--no-ignore"),
        (args.large, "--large", args.no_large, "--no-large"),
        (args.deps, "--deps", args.no_deps, "--no-deps"),
        (args.max_file_size.is_some(), "--max-file-size", args.large, "--large"),
        (args.max_file_size.is_some(), "--max-file-size", args.no_large, "--no-large"),
    ];
    if let Some((_, a, _, b)) = conflicts.iter().find(|(a, _, b, _)| *a && *b) {
        return Err(format!("{a} cannot be used with {b}"));
    }
    if args.paths.is_empty() {
        args.paths.push(PathBuf::from("."));
    }
    Ok(Parsed::Run(args))
}

/// The value of `flag`, parsed as a number.
fn number<T: FromStr>(parser: &mut lexopt::Parser, flag: &str) -> Result<T, String> {
    let value = parser.value().map_err(describe)?;
    match value.to_str().map(str::parse) {
        Some(Ok(number)) => Ok(number),
        _ => Err(invalid(&value, flag, "which takes a whole number")),
    }
}

fn invalid(value: &std::ffi::OsStr, flag: &str, expected: &str) -> String {
    format!("invalid value '{}' for {flag}, {expected}", value.to_string_lossy())
}

/// The message of a usage error that `lexopt` finds.
fn describe(error: lexopt::Error) -> String {
    match error {
        lexopt::Error::MissingValue { option: Some(option) } => format!("{option} needs a value"),
        lexopt::Error::UnexpectedValue { option, .. } => format!("{option} does not take a value"),
        error => error.to_string(),
    }
}

/// The message for an unknown long flag, which suggests the closest flag in the help when it is
/// at most two edits away.
fn unknown(flag: &str) -> String {
    let names =
        HELP.split_whitespace().filter_map(|word| word.strip_prefix("--")).flat_map(|word| {
            let (pair, name) = match word.strip_prefix("[no-]") {
                Some(name) => (true, name),
                None => (false, word),
            };
            let name = name.trim_end_matches(|c: char| !c.is_ascii_alphanumeric());
            [Some(name.to_string()), pair.then(|| format!("no-{name}"))].into_iter().flatten()
        });
    let closest = names.map(|name| (distance(flag, &name), name)).min_by_key(|(d, _)| *d);
    match closest {
        Some((d, name)) if d <= 2 => format!("unknown flag --{flag}, did you mean --{name}?"),
        _ => format!("unknown flag --{flag}"),
    }
}

/// The Levenshtein distance between `a` and `b`, in characters.
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut above: Vec<usize> = (0..=b.len()).collect();
    for (i, x) in a.chars().enumerate() {
        let mut row = vec![i + 1];
        for ((diagonal, up), y) in above.iter().zip(above.iter().skip(1)).zip(&b) {
            let left = row.last().copied().unwrap_or(0);
            row.push((diagonal + usize::from(x != *y)).min(up + 1).min(left + 1));
        }
        above = row;
    }
    above.last().copied().unwrap_or(0)
}
