// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! #[cfg(test)] block detection.
//!
//! Finds the inline `#[cfg(test)] mod` blocks of a Rust source file, lexing only the lines from
//! each attribute that may be `#[cfg(test)]` to the end of its item.

use std::ops::Range;

use memchr::{memchr, memchr_iter, memchr2, memmem};

use crate::cloc::{self, CodeLines, LineCounts, trim};

/// State for tracking multi-line attribute parsing.
struct MultiLineAttr {
    /// Accumulated content of the attribute.
    content: Vec<u8>,
    /// The offset of the line where the attribute started.
    start: usize,
}

/// The counts of the lines of a Rust source file, and the counts of the lines of its inline test
/// modules, which are a part of the first. Each module is counted as a text of its own.
pub fn count(content: &[u8]) -> (LineCounts, LineCounts) {
    let content = content.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(content);
    let mut code_starts = Vec::new();
    let counts = cloc::count_in(content, "rs", Some(&mut code_starts));
    // The counts of the lines before `offset`, when a line in `code_starts` or the end is there.
    let before = |offset: usize| match code_starts.binary_search_by_key(&offset, |&(at, _)| at) {
        Ok(i) => code_starts.get(i).map(|&(_, counts)| counts),
        Err(_) => (offset == content.len()).then_some(counts),
    };
    let mut test = LineCounts::default();
    for span in test_spans(content, &code_starts) {
        // A module that starts in code outside every comment and string has the same counts in
        // the file as alone, which are those before the line after it less those before it.
        test.add(match (before(span.start), before(span.end)) {
            (Some(first), Some(after)) => after.without(first),
            _ => cloc::count_lines(content.get(span).unwrap_or_default(), "rs"),
        });
    }
    (counts, test)
}

/// The byte ranges of the inline test modules of `content`, a Rust source file, each from the
/// start of its first line to the start of the line after its last.
///
/// A test module is a `mod` item after a `#[cfg(...)]` attribute that names `test`, which may span
/// lines, and it ends at the brace that brings the depth of the braces outside comments and
/// strings back to 0. A module declared without a body, as in `mod tests;`, is not one.
///
/// Lexing starts afresh at the lines in `code_starts`, which [`cloc::count_in`] pushes, and skips
/// to the last of them before the next byte that matters. Only the lines from there to that byte
/// are lexed.
pub fn test_spans(content: &[u8], code_starts: &[(usize, LineCounts)]) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    let mut in_cfg_test = false;
    let mut brace_depth: i32 = 0;
    let mut block_start = 0;
    let mut pending_attr: Option<MultiLineAttr> = None;
    // Whether the first opening brace after the attribute is still to come.
    let mut waiting_for_block_start = false;
    // Whether the current #[cfg(test)] item is a module
    let mut is_mod = false;
    // The offset of the line after the last line lexed, and those of the next `#` that may start
    // `#[cfg(` and of the next brace once they have been found.
    let Some(first) = next_cfg(content, 0) else { return spans };
    let (mut next, mut hash, mut brace) = (0, Some(first), None);
    let mut starts = code_starts.iter().map(|&(start, _)| start).peekable();
    let mut lines = CodeLines::new(content, "rs");
    loop {
        // Outside an attribute and a test item, only a line with `#[cfg(` in its code matters,
        // and inside the body of a test item, only a line with a brace. A line that ends before
        // `target`, the offset of the next such byte, is not split into code.
        let mut target = 0;
        if pending_attr.is_none() && !waiting_for_block_start {
            let found = if in_cfg_test { &mut brace } else { &mut hash };
            target = match *found {
                Some(at) if at >= next => at,
                _ => {
                    let at = if in_cfg_test {
                        content
                            .get(next..)
                            .and_then(|rest| memchr2(b'{', b'}', rest))
                            .map(|at| next + at)
                    } else {
                        next_cfg(content, next)
                    };
                    let Some(at) = at else { break };
                    *found.insert(at)
                }
            };
            let mut restart = None;
            while let Some(start) = starts.next_if(|&start| start <= target) {
                restart = Some(start).filter(|&start| start >= next).or(restart);
            }
            if let Some(start) = restart {
                lines.restart(content.get(start..).unwrap_or_default());
            }
        }
        let Some(line) = lines.next_line() else { break };
        let line_start = line.as_ptr().addr() - content.as_ptr().addr();
        next = (line_start + line.len() + 1).min(content.len());
        if target >= next {
            continue;
        }
        let trimmed = trim(lines.code());

        // If entire line is in a comment or a string, skip it
        if trimmed.is_empty() {
            continue;
        }

        // Handle multi-line attribute accumulation
        if let Some(ref mut attr) = pending_attr {
            attr.content.extend_from_slice(trimmed);

            // Check if the attribute is complete (has closing bracket)
            if contains(trimmed, b")]")
                || (trimmed.contains(&b')') && contains(&attr.content, b")]"))
            {
                // Attribute complete, check if it's cfg(test)
                if is_cfg_test_content(&attr.content) {
                    in_cfg_test = true;
                    waiting_for_block_start = true;
                    block_start = attr.start;
                    brace_depth = 0;
                }
                pending_attr = None;
            }
            continue;
        }

        // Check for #[cfg(test)] attribute (single-line or start of multi-line)
        if !in_cfg_test && let Some(attr_state) = detect_cfg_attr_start(trimmed, line_start) {
            match attr_state {
                CfgAttrState::Complete(is_test) => {
                    if is_test {
                        in_cfg_test = true;
                        waiting_for_block_start = true;
                        block_start = line_start;
                        brace_depth = 0;
                    }
                }
                CfgAttrState::Incomplete(attr) => {
                    pending_attr = Some(attr);
                }
            }
            if pending_attr.is_some() || in_cfg_test {
                continue;
            }
        }

        if in_cfg_test {
            // Skip additional attributes (like #[path = "..."])
            if trimmed.starts_with(b"#[") {
                continue;
            }

            let delta = count_braces(trimmed);

            // Before its first opening brace, an item that ends with `;` is declared without a
            // body, as an external module such as `mod tests;` is, and is not an inline test.
            if waiting_for_block_start {
                is_mod = is_mod_item(trimmed);

                if delta > 0 {
                    // Found opening brace - this is an inline block
                    waiting_for_block_start = false;
                    brace_depth += delta;
                } else if trimmed.ends_with(b";") {
                    // External module declaration (e.g., "mod tests;")
                    // Not an inline test block
                    in_cfg_test = false;
                    waiting_for_block_start = false;
                    continue;
                }
                // Otherwise keep waiting (might be blank line or comment)
                continue;
            }

            brace_depth += delta;

            if brace_depth == 0 && delta < 0 {
                // The item ends at the closing brace that brings the depth back to 0. Only `mod`
                // blocks count as tests. Other items, such as helpers, fixtures and macros, are
                // test infrastructure.
                if is_mod {
                    spans.push(block_start..next);
                }
                in_cfg_test = false;
            }
        }
    }
    spans
}

/// The offset of the first `#` in `content` at or after `from` that may start `#[cfg(` in the
/// code of its line.
///
/// The code of a line joins its runs outside comments and strings, in order. A comment or string
/// that splits `#[cfg(` opens with `/`, `"` or `'` right after the part of `#[cfg(` before it, and
/// the rest of `#[cfg(` follows it in order on the line.
fn next_cfg(content: &[u8], mut from: usize) -> Option<usize> {
    const CFG: &[u8] = b"#[cfg(";
    loop {
        let hash = from + memchr(b'#', content.get(from..)?)?;
        let text = content.get(hash..)?;
        let same = text.iter().zip(CFG).take_while(|(byte, expected)| byte == expected).count();
        let rest = CFG.get(same..).unwrap_or_default();
        if rest.is_empty()
            || matches!(text.get(same), Some(b'/' | b'"' | b'\'')) && {
                let mut line = text.iter().skip(same).take_while(|&&byte| byte != b'\n');
                rest.iter().all(|expected| line.any(|byte| byte == expected))
            }
        {
            return Some(hash);
        }
        from = hash + 1;
    }
}

/// Result of detecting a cfg attribute start.
enum CfgAttrState {
    /// Complete single-line attribute, bool indicates if it's cfg(test).
    Complete(bool),
    /// Incomplete multi-line attribute that needs more lines.
    Incomplete(MultiLineAttr),
}

/// Whether `needle` occurs in `haystack`.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    memmem::find(haystack, needle).is_some()
}

/// Detect if a line starts a #[cfg(...)] attribute.
/// Returns the state of the attribute parsing.
fn detect_cfg_attr_start(line: &[u8], start: usize) -> Option<CfgAttrState> {
    // Check for #[cfg( pattern
    if !memchr_iter(b'#', line)
        .any(|i| line.get(i..).is_some_and(|rest| rest.starts_with(b"#[cfg(")))
    {
        return None;
    }

    Some({
        // Check if the attribute is complete on this line
        if contains(line, b")]") {
            // Single-line case
            CfgAttrState::Complete(is_cfg_test_content(line))
        } else {
            // Multi-line case - attribute continues on next line(s)
            CfgAttrState::Incomplete(MultiLineAttr { content: line.to_vec(), start })
        }
    })
}

/// Check if accumulated cfg attribute content contains "test".
fn is_cfg_test_content(content: &[u8]) -> bool {
    // Look for cfg(test) pattern with optional whitespace
    // The content may be the full attribute like "#[cfg(test)]"
    // or accumulated multi-line content like "#[cfg(\n    test\n)]"

    // Extract the part between #[cfg( and )]
    const OPEN: &[u8] = b"#[cfg(";
    let Some(start) = memmem::find(content, OPEN) else { return false };
    let Some(after_cfg) = content.get(start + OPEN.len()..) else { return false };
    let Some((inner, _)) =
        memmem::find(after_cfg, b")]").and_then(|end| after_cfg.split_at_checked(end))
    else {
        return false;
    };
    // The inner text holds `test` as a standalone word in cases such as `test`, ` test ` and
    // `all(test, ...)`. Words are split by characters rather than bytes. Attributes are few and
    // short, so decoding them costs little.
    String::from_utf8_lossy(inner)
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .any(|word| word == "test")
}

/// Whether a line following `#[cfg(test)]` declares a module, after any visibility.
fn is_mod_item(line: &[u8]) -> bool {
    let line = trim(line);
    let line = match line.strip_prefix(b"pub(") {
        Some(rest) => match rest.splitn(2, |&b| b == b')').nth(1) {
            Some(after_close) => trim(after_close),
            None => line,
        },

        None => line.strip_prefix(b"pub ").unwrap_or(line),
    };
    line == b"mod" || line.starts_with(b"mod ")
}

/// The change in brace depth over a line of code.
fn count_braces(code: &[u8]) -> i32 {
    code.iter().map(|&b| i32::from(b == b'{') - i32::from(b == b'}')).sum()
}

#[cfg(test)]
#[path = "cfg_test_tests.rs"]
mod tests;
