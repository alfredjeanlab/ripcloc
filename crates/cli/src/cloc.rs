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
        "make" => "Makefile",
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

/// Whether files with the key `key` are split into source and tests. Documentation,
/// configuration and build files are not.
pub fn is_split(key: &str) -> bool {
    !matches!(key, "md" | "markdown" | "yaml" | "yml" | "toml" | "make" | "dockerfile")
}

/// Whether files with the extension `ext` are counted.
pub fn is_counted(ext: &str) -> bool {
    comment_style(ext).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_single_line_comments() {
        let content = "// this is a comment\nfn main() {}\n// another comment\n";
        let counts = count_lines(content, "rs");
        assert_eq!(counts, LineCounts { blank: 0, comment: 2, code: 1 });
    }

    #[test]
    fn rust_block_comment() {
        let content = "/* block\n   comment */\nfn main() {}\n";
        let counts = count_lines(content, "rs");
        assert_eq!(counts, LineCounts { blank: 0, comment: 2, code: 1 });
    }

    #[test]
    fn rust_mixed_blank_comment_code() {
        let content = "\n// comment\n\nfn foo() {}\n\n// end\n";
        let counts = count_lines(content, "rs");
        assert_eq!(counts, LineCounts { blank: 3, comment: 2, code: 1 });
    }

    #[test]
    fn rust_block_comment_with_code_after_close() {
        let content = "/* comment */ let x = 1;\n";
        let counts = count_lines(content, "rs");
        // Opening line has code after close -> code
        assert_eq!(counts, LineCounts { blank: 0, comment: 0, code: 1 });
    }

    #[test]
    fn rust_multiline_block_with_code_after_close() {
        // Line 1: "/* start" -> opens block comment (comment)
        // Line 2: "  middle" -> inside block (comment)
        // Line 3: "  end */ let x = 1;" -> closes block, code after (code)
        // Line 4: "fn foo() {}" -> code
        let content = "/* start\n  middle\n  end */ let x = 1;\nfn foo() {}\n";
        let counts = count_lines(content, "rs");
        assert_eq!(counts, LineCounts { blank: 0, comment: 2, code: 2 });
    }

    #[test]
    fn python_hash_comments() {
        let content = "# comment\ndef foo():\n    # inline\n    pass\n";
        let counts = count_lines(content, "py");
        assert_eq!(counts, LineCounts { blank: 0, comment: 2, code: 2 });
    }

    #[test]
    fn python_no_block_comments() {
        // Python doesn't have block comments in our model
        let content = "# comment\ndef foo():\n    pass\n";
        let counts = count_lines(content, "py");
        assert_eq!(counts, LineCounts { blank: 0, comment: 1, code: 2 });
    }

    #[test]
    fn go_comments() {
        let content = "// Package main\npackage main\n\n/* multi\n   line */\nfunc main() {}\n";
        let counts = count_lines(content, "go");
        assert_eq!(counts, LineCounts { blank: 1, comment: 3, code: 2 });
    }

    #[test]
    fn lua_comments() {
        let content = "-- single line\n--[[ block\ncomment ]]\nlocal x = 1\n";
        let counts = count_lines(content, "lua");
        assert_eq!(counts, LineCounts { blank: 0, comment: 3, code: 1 });
    }

    #[test]
    fn php_dual_line_comments() {
        let content = "// C-style\n# shell-style\n$x = 1;\n/* block */\n";
        let counts = count_lines(content, "php");
        assert_eq!(counts, LineCounts { blank: 0, comment: 3, code: 1 });
    }

    #[test]
    fn empty_content() {
        let counts = count_lines("", "rs");
        assert_eq!(counts, LineCounts { blank: 0, comment: 0, code: 0 });
    }

    #[test]
    fn all_blank() {
        let counts = count_lines("\n\n\n", "rs");
        assert_eq!(counts, LineCounts { blank: 3, comment: 0, code: 0 });
    }

    #[test]
    fn unknown_extension() {
        assert!(comment_style("xyz").is_none());
    }

    #[test]
    fn single_line_block_comment() {
        let content = "/* single-line block comment */\ncode();\n";
        let counts = count_lines(content, "rs");
        assert_eq!(counts, LineCounts { blank: 0, comment: 1, code: 1 });
    }

    #[test]
    fn nested_looking_block_comment() {
        // We don't support true nesting; just find the close delimiter
        let content = "/* outer /* inner */ code();\n";
        let counts = count_lines(content, "rs");
        // The first `*/` closes the block, `code();` is code on same line
        assert_eq!(counts, LineCounts { blank: 0, comment: 0, code: 1 });
    }

    #[test]
    fn bat_rem_comment() {
        let content = "REM this is a comment\n:: another comment\necho hello\n";
        let counts = count_lines(content, "bat");
        assert_eq!(counts, LineCounts { blank: 0, comment: 2, code: 1 });
    }

    #[test]
    fn vue_html_comment() {
        let content = "<!-- comment -->\n<template>\n</template>\n";
        let counts = count_lines(content, "vue");
        assert_eq!(counts, LineCounts { blank: 0, comment: 1, code: 2 });
    }

    #[test]
    fn perl_pod_block() {
        let content = "=pod\nDocumentation here\n=cut\nmy $x = 1;\n";
        let counts = count_lines(content, "pl");
        assert_eq!(counts, LineCounts { blank: 0, comment: 3, code: 1 });
    }

    #[test]
    fn powershell_block_comment() {
        let content = "<#\nblock comment\n#>\n$x = 1\n";
        let counts = count_lines(content, "ps1");
        assert_eq!(counts, LineCounts { blank: 0, comment: 3, code: 1 });
    }

    #[test]
    fn unknown_extension_counts_every_nonblank_line_as_code() {
        assert_eq!(
            count_lines("line 1\n\n# line 3\n", "xyz"),
            LineCounts { blank: 1, comment: 0, code: 2 }
        );
    }

    #[test]
    fn language_names() {
        assert_eq!(language_name("rs"), "Rust");
        assert_eq!(language_name("jsx"), "JavaScript");
        assert_eq!(language_name("tsx"), "TypeScript");
        assert_eq!(language_name("bash"), "Shell");
        assert_eq!(language_name("xyz"), "xyz");
    }

    #[test]
    fn counts_source_and_text_formats_but_not_data() {
        assert!(
            ["rs", "py", "tsx", "mjs", "sh", "toml", "md", "json", "yaml"]
                .iter()
                .all(|e| is_counted(e))
        );
        assert!(!["csv", "lock", "txt", "png"].iter().any(|e| is_counted(e)));
    }

    #[test]
    fn file_keys_recognize_names_without_extensions() {
        use std::path::Path;
        assert_eq!(file_key(Path::new("a/Makefile")), "make");
        assert_eq!(file_key(Path::new("Dockerfile")), "dockerfile");
        assert_eq!(file_key(Path::new("x/CMakeLists.txt")), "cmake");
        assert_eq!(file_key(Path::new("src/Main.RS")), "rs");
    }

    #[test]
    fn json_has_no_comments_and_markdown_uses_html_comments() {
        assert_eq!(
            count_lines("{\n\n  \"a\": 1\n}\n", "json"),
            LineCounts { blank: 1, comment: 0, code: 3 }
        );
        assert_eq!(
            count_lines("# Title\n<!-- note -->\ntext\n", "md"),
            LineCounts { blank: 0, comment: 1, code: 2 }
        );
    }
}
