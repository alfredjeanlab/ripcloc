//! The colors of the text report and of the help, which are shown only on a terminal.

use std::io::IsTerminal;

/// The color of section headings and of the report's column header.
pub const HEADER: u8 = 74;
/// The color of flags in the help, and of the rows that break down the row above them.
pub const LITERAL: u8 = 250;
/// The color of rules, defaults and hints.
pub const CONTEXT: u8 = 245;

/// Whether to color stdout.
///
/// `NO_COLOR` disables color and `COLOR` forces it, whatever their values. Otherwise color is
/// used only on a terminal outside an agent or CI environment.
pub fn enabled() -> bool {
    let set = |name: &str| std::env::var_os(name).is_some();
    if set("NO_COLOR") {
        return false;
    }
    set("COLOR")
        || (std::io::stdout().is_terminal()
            && !["CLAUDE_CODE", "CODEX", "CURSOR", "CI"].into_iter().any(set))
}

/// `text` in the 256-color `code` when `on`, or else `text` as it is.
pub fn paint(on: bool, code: u8, text: &str) -> String {
    if on { format!("\x1b[38;5;{code}m{text}\x1b[0m") } else { text.to_string() }
}

/// The help text with its section headings, its flags and its defaults colored.
///
/// A heading is the word before the first `:` of a line that starts without indentation, a flag
/// column is the start of an indented line that begins with `-` or `[`, up to the gap before its
/// description, and a default is each `[default: …]`.
pub fn help(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for line in text.split_inclusive('\n') {
        let (body, newline) = line.strip_suffix('\n').map_or((line, ""), |body| (body, "\n"));
        let colored = match body.find(':') {
            Some(end) if !body.starts_with(' ') && end > 0 => {
                let (heading, rest) = body.split_at(end + 1);
                format!("{}{}", paint(true, HEADER, heading), defaults(rest))
            }
            _ => match flag_column(body) {
                Some(end) => {
                    let (flags, rest) = body.split_at(end);
                    format!("{}{}", paint(true, LITERAL, flags), defaults(rest))
                }
                None => defaults(body),
            },
        };
        out.push_str(&colored);
        out.push_str(newline);
    }
    out
}

/// The offset in `line` after its flag column, if `line` is an indented line of flags.
fn flag_column(line: &str) -> Option<usize> {
    let start = line.len() - line.trim_start().len();
    let rest = line.get(start..)?;
    if start == 0 || !(rest.starts_with('-') || rest.starts_with('[')) {
        return None;
    }
    // The description follows a gap of at least two spaces.
    Some(rest.find("  ").map_or(line.len(), |gap| start + gap))
}

/// `text` with each `[default: …]` in it colored.
fn defaults(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("[default:") {
        let Some(len) = rest.get(start..).and_then(|tail| tail.find(']')) else { break };
        let (before, tail) = rest.split_at(start);
        let (default, after) = tail.split_at(len + 1);
        out.push_str(before);
        out.push_str(&paint(true, CONTEXT, default));
        rest = after;
    }
    out.push_str(rest);
    out
}
