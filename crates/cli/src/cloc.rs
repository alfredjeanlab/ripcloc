// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! Line counting with per-language comment detection.
//!
//! Comments are found line by line with simple heuristics rather than parsing, as `cloc` does.

/// Blank, comment and code line counts.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LineCounts {
    pub blank: usize,
    pub comment: usize,
    pub code: usize,
}

/// Comment syntax for a language.
pub struct CommentStyle {
    /// Single-line comment prefixes (e.g. `["//", "#"]`).
    pub line: &'static [&'static str],
    /// Block comment delimiter pairs `(open, close)`.
    pub block: &'static [(&'static str, &'static str)],
}

/// Return the comment style for a file extension, or `None` if unknown.
pub fn comment_style(ext: &str) -> Option<CommentStyle> {
    match ext {
        "rs" => Some(CommentStyle { line: &["//"], block: &[("/*", "*/")] }),
        "go" => Some(CommentStyle { line: &["//"], block: &[("/*", "*/")] }),
        "py" | "rb" | "sh" | "bash" | "zsh" | "fish" | "bats" | "r" => {
            Some(CommentStyle { line: &["#"], block: &[] })
        }
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "mts" | "cjs" | "cts" | "java" | "kt" | "scala"
        | "swift" | "c" | "cpp" | "h" | "hpp" | "cs" | "m" | "mm" => {
            Some(CommentStyle { line: &["//"], block: &[("/*", "*/")] })
        }
        "lua" => Some(CommentStyle { line: &["--"], block: &[("--[[", "]]")] }),
        "sql" => Some(CommentStyle { line: &["--"], block: &[("/*", "*/")] }),
        "php" => Some(CommentStyle { line: &["//", "#"], block: &[("/*", "*/")] }),
        "vue" | "svelte" => {
            Some(CommentStyle { line: &["//"], block: &[("/*", "*/"), ("<!--", "-->")] })
        }
        "pl" | "pm" => Some(CommentStyle { line: &["#"], block: &[("=pod", "=cut")] }),
        "ps1" => Some(CommentStyle { line: &["#"], block: &[("<#", "#>")] }),
        "bat" | "cmd" => Some(CommentStyle { line: &["REM ", ":: "], block: &[] }),
        "toml" | "yaml" | "yml" | "make" | "dockerfile" | "cmake" => {
            Some(CommentStyle { line: &["#"], block: &[] })
        }
        "json" => Some(CommentStyle { line: &[], block: &[] }),
        "md" | "markdown" | "html" | "htm" | "xml" => {
            Some(CommentStyle { line: &[], block: &[("<!--", "-->")] })
        }
        "css" => Some(CommentStyle { line: &[], block: &[("/*", "*/")] }),
        "scss" | "sass" | "less" | "proto" | "cc" | "cxx" | "hh" | "kts" => {
            Some(CommentStyle { line: &["//"], block: &[("/*", "*/")] })
        }
        "ini" => Some(CommentStyle { line: &[";", "#"], block: &[] }),
        _ => None,
    }
}

/// Count the lines of `content`, using the comment syntax for the extension `ext`.
///
/// A line that has code after the close of a block comment is code. Without a known comment
/// syntax, every non-blank line is code.
pub fn count_lines(content: &str, ext: &str) -> LineCounts {
    let style = comment_style(ext).unwrap_or(CommentStyle { line: &[], block: &[] });
    let mut counts = LineCounts::default();
    // The close delimiter of the block comment that the current line is inside.
    let mut in_block: Option<&'static str> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            counts.blank += 1;
            continue;
        }
        // The text after a block comment that closes on this line, if one does.
        let after_close = if let Some(close) = in_block {
            trimmed.find(close).map(|pos| &trimmed[pos + close.len()..])
        } else if let Some((rest, close)) =
            style.block.iter().find_map(|&(open, close)| Some((trimmed.strip_prefix(open)?, close)))
        {
            in_block = Some(close);
            rest.find(close).map(|pos| &rest[pos + close.len()..])
        } else if style.line.iter().any(|p| trimmed.starts_with(p)) {
            counts.comment += 1;
            continue;
        } else {
            counts.code += 1;
            continue;
        };
        match after_close {
            None => counts.comment += 1,
            Some(after) => {
                in_block = None;
                if after.trim().is_empty() {
                    counts.comment += 1;
                } else {
                    counts.code += 1;
                }
            }
        }
    }
    counts
}

/// Map file extension to human-readable language name.
pub fn language_name(ext: &str) -> &str {
    match ext {
        "rs" => "Rust",
        "go" => "Go",
        "py" => "Python",
        "rb" => "Ruby",
        "js" | "jsx" | "mjs" | "cjs" => "JavaScript",
        "ts" | "tsx" | "mts" | "cts" => "TypeScript",
        "java" => "Java",
        "kt" | "kts" => "Kotlin",
        "scala" => "Scala",
        "swift" => "Swift",
        "c" => "C",
        "cpp" | "hpp" | "cc" | "cxx" | "hh" => "C++",
        "h" => "C/C++ Header",
        "cs" => "C#",
        "m" => "Objective-C",
        "mm" => "Objective-C++",
        "sh" | "bash" | "zsh" | "fish" | "bats" => "Shell",
        "ps1" => "PowerShell",
        "bat" | "cmd" => "Batch",
        "php" => "PHP",
        "lua" => "Lua",
        "sql" => "SQL",
        "r" => "R",
        "pl" | "pm" => "Perl",
        "vue" => "Vue",
        "svelte" => "Svelte",
        "toml" => "TOML",
        "yaml" | "yml" => "YAML",
        "json" => "JSON",
        "md" | "markdown" => "Markdown",
        "html" | "htm" => "HTML",
        "xml" => "XML",
        "css" => "CSS",
        "scss" => "SCSS",
        "sass" => "Sass",
        "less" => "LESS",
        "ini" => "INI",
        "proto" => "Protocol Buffers",
        "make" => "make",
        "dockerfile" => "Dockerfile",
        "cmake" => "CMake",
        other => other,
    }
}

/// The key that the tables in this module use for a file: a few names that have no extension,
/// such as `Makefile`, or else the lowercased extension.
pub fn file_key(path: &std::path::Path) -> String {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    match name {
        "Makefile" | "makefile" | "GNUmakefile" => "make".to_string(),
        "Dockerfile" => "dockerfile".to_string(),
        "CMakeLists.txt" => "cmake".to_string(),
        _ => path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase(),
    }
}

/// Whether files with the extension `ext` are counted.
pub fn is_counted(ext: &str) -> bool {
    comment_style(ext).is_some()
}

#[cfg(test)]
#[path = "cloc_tests.rs"]
mod tests;
