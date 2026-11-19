// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! #[cfg(test)] block detection.
//!
//! Parses Rust source files to identify line ranges inside #[cfg(test)] blocks.

use std::borrow::Cow;
use std::ops::Range;

use memchr::{memchr_iter, memmem};

use crate::cloc::{lines, trim, utf8_width};

/// Lexer state for tracking what context we're in.
#[derive(Debug, Clone, Copy, PartialEq)]
enum LexerState {
    /// Normal code - braces count
    Code,
    /// Inside a regular string "..."
    String,
    /// Inside a raw string r"..." or r#"..."#
    /// The usize is the number of # delimiters
    RawString(usize),
    /// Inside a character literal '...'
    Char,
}

/// Result of parsing a Rust file for #[cfg(test)] blocks.
#[derive(Debug, Default)]
pub struct CfgTestInfo {
    /// Line ranges (0-indexed) of `#[cfg(test)] mod` blocks, from the attribute through the
    /// closing brace.
    pub test_ranges: Vec<Range<usize>>,
    /// The number of non-blank lines in `test_ranges`.
    pub test_lines: usize,
}

/// State for tracking multi-line attribute parsing.
struct MultiLineAttr {
    /// Accumulated content of the attribute.
    content: Vec<u8>,
    /// Line where the attribute started.
    start_line: usize,
    /// The number of non-blank lines before `start_line`.
    nonblank_before: usize,
}

impl CfgTestInfo {
    /// Parse a Rust source file to find #[cfg(test)] block ranges.
    ///
    /// Uses a brace-counting approach with proper lexer state tracking:
    /// 1. Scan for #[cfg(test)] attribute (handles multi-line)
    /// 2. Count { and } to track block depth (skipping string/char literals)
    /// 3. Block ends when brace depth returns to 0
    ///
    /// External module declarations (`mod tests;`) are NOT counted as inline tests.
    pub fn parse(content: &(impl AsRef<[u8]> + ?Sized)) -> Self {
        let mut info = Self::default();
        let mut in_cfg_test = false;
        let mut brace_depth: i32 = 0;
        let mut block_start = 0;
        let mut pending_attr: Option<MultiLineAttr> = None;
        // Track if we've seen the first opening brace after #[cfg(test)]
        let mut waiting_for_block_start = false;
        // Track if we're inside a block comment
        let mut in_block_comment = false;
        // Whether the current #[cfg(test)] item is a module
        let mut is_mod = false;
        // The number of non-blank lines so far, and before `block_start`.
        let mut nonblank = 0;
        let mut nonblank_before = 0;

        for (line_idx, line) in lines(content.as_ref()).enumerate() {
            let trimmed = trim(line);
            if trimmed.is_empty() {
                continue;
            }
            nonblank += 1;

            // Track block comment state and strip commented portions
            let code_part = strip_comments(trimmed, &mut in_block_comment);
            let trimmed = trim(&code_part);

            // If entire line is in comment, skip it
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
                        block_start = attr.start_line;
                        nonblank_before = attr.nonblank_before;
                        brace_depth = 0;
                    }
                    pending_attr = None;
                }
                continue;
            }

            // Check for #[cfg(test)] attribute (single-line or start of multi-line)
            if !in_cfg_test
                && let Some(attr_state) = detect_cfg_attr_start(trimmed, line_idx, nonblank - 1)
            {
                match attr_state {
                    CfgAttrState::Complete(is_test) => {
                        if is_test {
                            in_cfg_test = true;
                            waiting_for_block_start = true;
                            block_start = line_idx;
                            nonblank_before = nonblank - 1;
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

                // If we're still waiting for the block to start and we see a line
                // without an opening brace that ends with ';', it's an external module
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
                    // Block ended (we saw a closing brace that brought us to 0)
                    // Only `mod` blocks count as tests. Other items, such as helpers, fixtures
                    // and macros, are test infrastructure.
                    if is_mod {
                        info.test_ranges.push(block_start..line_idx + 1);
                        info.test_lines += nonblank - nonblank_before;
                    }
                    in_cfg_test = false;
                }
            }
        }

        info
    }

    /// Check if a line (0-indexed) is inside a #[cfg(test)] block.
    #[cfg(test)]
    pub fn is_test_line(&self, line_idx: usize) -> bool {
        self.test_ranges.iter().any(|r| r.contains(&line_idx))
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

/// Strip comments from a line, handling both line comments and block comments.
/// Updates the `in_block_comment` state for multi-line block comments.
/// Returns the code portion of the line (empty if entire line is comment).
fn strip_comments<'a>(line: &'a [u8], in_block_comment: &mut bool) -> Cow<'a, [u8]> {
    // Most lines have no comment, or end with a line comment, and need no copy.
    if !*in_block_comment {
        let comment = memchr_iter(b'/', line).find_map(|i| match line.get(i + 1) {
            Some(b'/') => Some((i, false)),
            Some(b'*') => Some((i, true)),
            _ => None,
        });
        match comment {
            None => return Cow::Borrowed(line),
            Some((i, false)) => return Cow::Borrowed(&line[..i]),
            Some((_, true)) => {}
        }
    }

    let mut result = Vec::new();
    let mut i = 0;
    while let Some(&ch) = line.get(i) {
        i += 1;
        if *in_block_comment {
            // Looking for end of block comment */
            if ch == b'*' && line.get(i) == Some(&b'/') {
                i += 1; // consume '/'
                *in_block_comment = false;
            }
            // Skip everything inside block comment
        } else if ch == b'/' {
            match line.get(i) {
                Some(b'/') => {
                    // Line comment - rest of line is comment
                    break;
                }
                Some(b'*') => {
                    // Start of block comment
                    i += 1; // consume '*'
                    *in_block_comment = true;
                }
                _ => {
                    // Just a regular '/' character
                    result.push(ch);
                }
            }
        } else {
            result.push(ch);
        }
    }

    Cow::Owned(result)
}

/// Detect if a line starts a #[cfg(...)] attribute.
/// Returns the state of the attribute parsing.
fn detect_cfg_attr_start(
    line: &[u8],
    line_idx: usize,
    nonblank_before: usize,
) -> Option<CfgAttrState> {
    // Check for #[cfg( pattern
    if !memchr_iter(b'#', line).any(|i| line[i..].starts_with(b"#[cfg(")) {
        return None;
    }

    Some({
        // Check if the attribute is complete on this line
        if contains(line, b")]") {
            // Single-line case
            CfgAttrState::Complete(is_cfg_test_content(line))
        } else {
            // Multi-line case - attribute continues on next line(s)
            CfgAttrState::Incomplete(MultiLineAttr {
                content: line.to_vec(),
                start_line: line_idx,
                nonblank_before,
            })
        }
    })
}

/// Check if accumulated cfg attribute content contains "test".
fn is_cfg_test_content(content: &[u8]) -> bool {
    // Look for cfg(test) pattern with optional whitespace
    // The content may be the full attribute like "#[cfg(test)]"
    // or accumulated multi-line content like "#[cfg(\n    test\n)]"

    // Extract the part between #[cfg( and )]
    if let Some(start) = memmem::find(content, b"#[cfg(") {
        let after_cfg = &content[start + 6..];
        if let Some(end) = memmem::find(after_cfg, b")]") {
            // Words are split by characters rather than bytes. Attributes are few and short, so
            // decoding them costs little.
            let inner = String::from_utf8_lossy(&after_cfg[..end]);
            // Check if inner contains "test" as a standalone word
            // Handle cases like "test", " test ", "all(test, ...)", etc.
            return inner
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .any(|word| word == "test");
        }
    }
    false
}

/// Whether a line following `#[cfg(test)]` declares a module, after any visibility.
fn is_mod_item(line: &[u8]) -> bool {
    let line = trim(line);
    let line = match line.strip_prefix(b"pub(") {
        Some(rest) => match rest.iter().position(|&b| b == b')') {
            Some(close) => trim(&rest[close + 1..]),
            None => line,
        },
        None => line.strip_prefix(b"pub ").unwrap_or(line),
    };
    line == b"mod" || line.starts_with(b"mod ")
}

/// Count brace depth changes in a line, accounting for string/char literals.
fn count_braces(line: &[u8]) -> i32 {
    let mut depth_change: i32 = 0;
    let mut state = LexerState::Code;
    let mut i = 0;
    let mut prev = 0;
    // The index after the character at `at`, which is `at` itself past the end of the line.
    let skip_char = |at: usize| at + line.get(at).map_or(0, |&b| utf8_width(b));

    while let Some(&ch) = line.get(i) {
        i += 1;
        match state {
            LexerState::Code => {
                match ch {
                    b'"' => {
                        state = LexerState::String;
                    }
                    b'r' => {
                        // Check for raw string: r"..." or r#"..."#
                        if line.get(i) == Some(&b'"') {
                            i += 1; // consume "
                            state = LexerState::RawString(0);
                        } else if line.get(i) == Some(&b'#') {
                            // Count consecutive #s
                            let mut hash_count = 0;
                            while line.get(i) == Some(&b'#') {
                                i += 1;
                                hash_count += 1;
                            }
                            // Must be followed by "
                            if line.get(i) == Some(&b'"') {
                                i += 1;
                                state = LexerState::RawString(hash_count);
                            }
                        }
                    }
                    b'\'' => {
                        // Character literal - but be careful about lifetimes
                        // Lifetime syntax: 'a, 'static, etc.
                        // Char literal: 'x', '\n', '\''
                        // Look ahead by characters, not bytes, to determine which
                        let end = match line.get(i) {
                            // Escape sequence: '\n', '\'', etc.
                            Some(b'\\') => Some(skip_char(i + 1)),
                            // Single character 'x'
                            Some(_) => Some(skip_char(i)),
                            None => None,
                        };
                        if end.is_some_and(|end| line.get(end) == Some(&b'\'')) {
                            state = LexerState::Char;
                        }
                    }
                    b'{' => depth_change += 1,
                    b'}' => depth_change -= 1,
                    _ => {}
                }
            }
            LexerState::String => {
                if ch == b'"' && prev != b'\\' {
                    state = LexerState::Code;
                }
            }
            LexerState::RawString(hash_count) => {
                // Raw string ends with "### where # count matches
                if ch == b'"' {
                    let mut matched = 0;
                    while matched < hash_count && line.get(i) == Some(&b'#') {
                        i += 1;
                        matched += 1;
                    }
                    if matched == hash_count {
                        state = LexerState::Code;
                    }
                }
            }
            LexerState::Char => {
                // Char literal ends at closing '
                if ch == b'\'' && prev != b'\\' {
                    state = LexerState::Code;
                }
            }
        }
        prev = ch;
    }

    depth_change
}

/// The line ranges (0-indexed) of the inline test modules of a Rust source file.
///
/// A file without `cfg(` has no test modules, unless a block comment splits the attribute, as in
/// `#[c/* */fg(test)]`, so only a file with one or the other is parsed.
pub fn test_ranges(content: &[u8]) -> Vec<Range<usize>> {
    if !contains(content, b"cfg(") && !contains(content, b"/*") {
        return Vec::new();
    }
    CfgTestInfo::parse(content).test_ranges
}

/// The byte ranges of the inline test modules of a Rust source file, each from the start of its
/// first line to the start of the line after its last.
pub fn test_spans(content: &[u8]) -> Vec<Range<usize>> {
    let ranges = test_ranges(content);
    if ranges.is_empty() {
        return ranges;
    }
    let mut starts = memchr_iter(b'\n', content).map(|i| i + 1);
    let (mut line, mut offset) = (0, 0);
    // The ranges are in order, so the line starts are found in one scan.
    let mut start_of = |target: usize| {
        while line < target {
            offset = starts.next().unwrap_or(content.len());
            line += 1;
        }
        offset
    };
    ranges.into_iter().map(|r| start_of(r.start)..start_of(r.end)).collect()
}

#[cfg(test)]
#[path = "cfg_test_tests.rs"]
mod tests;
