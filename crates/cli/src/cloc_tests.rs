// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

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
