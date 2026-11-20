// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! #[cfg(test)] block detection.
//!
//! Parses Rust source files to identify line ranges inside #[cfg(test)] blocks.

use std::ops::Range;

use memchr::{memchr_iter, memmem};

use crate::cloc::{code_lines, trim};

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
        // Whether the current #[cfg(test)] item is a module
        let mut is_mod = false;
        // The number of non-blank lines so far, and before `block_start`.
        let mut nonblank = 0;
        let mut nonblank_before = 0;

        for (line_idx, (line, code)) in code_lines(content.as_ref(), "rs").enumerate() {
            if trim(line).is_empty() {
                continue;
            }
            nonblank += 1;

            let trimmed = trim(&code);

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

/// Detect if a line starts a #[cfg(...)] attribute.
/// Returns the state of the attribute parsing.
fn detect_cfg_attr_start(
    line: &[u8],
    line_idx: usize,
    nonblank_before: usize,
) -> Option<CfgAttrState> {
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
