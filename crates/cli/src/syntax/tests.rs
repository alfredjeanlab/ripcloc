// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

use super::languages::{D_PREFIX, PL_PREFIX, V_PREFIX};
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
fn block_comments_nest_where_the_language_nests_them() {
    let content =
        "/* outer /* inner */ \" still\n// in\n*/\nfn f() {} /* a /* b */\n\"\n*/\n// c\n";
    assert_eq!(counts(content, "rs"), (0, 6, 1));
    assert_eq!(counts(content, "swift"), (0, 6, 1));
    // In C the first `*/` closes the comment.
    assert_eq!(counts("/* outer /* inner */ code();\n", "c"), (0, 0, 1));
}

#[test]
fn escaped_character_literals_end_at_their_quote() {
    let content =
        "fn f(c: char) -> bool { matches!(c, '\\x22'|'\"') }\n// one\n// two\nfn g() {}\n";
    assert_eq!(counts(content, "rs"), (0, 2, 2));
    let content = "let a = '\\u{7b}'; let b = '\"';\n// c\n";
    assert_eq!(counts(content, "rs"), (0, 1, 1));
}

#[test]
fn javascript_regular_expressions_follow_arrows() {
    let content = "const f = xs.filter(s => /`/.test(s));\n// one\n// two\nconst g = 1;\n";
    assert_eq!(counts(content, "js"), (0, 2, 2));
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

// (blank, comment, code)
#[yare::parameterized(
    html_script_and_style = {
        "<html>\n<script>\n// one\nlet a = 1; /* two */\n</script>\n<style>\n/* three\n   four */\nb { c: d }\n</style>\n</html>\n",
        "html", (0, 3, 8)
    },
    vue_typescript_and_scss = {
        "<template>\n  <p>// text</p>\n</template>\n\n<script setup lang=\"ts\">\n// one\nconst a: string = \"//\"\n</script>\n<style lang='scss'>\n// two\n</style>\n",
        "vue", (1, 2, 8)
    },
    svelte_script = { "<script lang=\"ts\">\n  /** one */\n  let a = 1\n</script>\n<h1>{a}</h1>\n", "svelte", (0, 1, 4) },
    tags_ignore_case = { "<SCRIPT TYPE=\"module\">\n// one\n</Script>\n", "htm", (0, 1, 2) },
    script_in_html_comment = { "<!-- <script>\n// two -->\n// text\n", "html", (0, 2, 1) },
    element_closed_on_its_line = { "<script>a()</script>\n// text\n", "html", (0, 0, 2) },
    script_without_body = { "<script src=\"a.js\" />\n// text\n", "vue", (0, 0, 2) },
    json_script = { "<script type=\"application/ld+json\">\n{ \"a\": \"//\" }\n</script>\n", "html", (0, 0, 3) },
    template_script = { "<script type=\"text/x-template\">\n<!-- one -->\n</script>\n", "html", (0, 1, 2) },
    unknown_language = { "<script lang=\"coffee\">\n# one\n</script>\n", "vue", (0, 0, 3) },
    elements_on_one_line = {
        "<style>\n/* one */\n</style><script>\n// two\n</script>\n", "vue", (0, 2, 3)
    },
    unclosed_element = { "<style>\n/* one */\n\n", "html", (1, 1, 1) },
)]
fn markup_embeds_languages(content: &str, ext: &str, expected: (usize, usize, usize)) {
    assert_eq!(counts(content, ext), expected);
}

#[test]
fn ruby_begin_end_block() {
    let content = "=begin\nThis is\ncommented out\n=end\nx = 1 # c\n";
    for ext in ["rb", "rake", "gemspec"] {
        assert_eq!(counts(content, ext), (0, 4, 1), "{ext}");
    }
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
fn lines_split_like_str_lines() {
    for text in ["", "\n", "a", "a\n", "a\nb", "a\r\nb\r\n", "\n\nx\n\n"] {
        let expected: Vec<&str> = text.lines().collect();
        let actual: Vec<&str> =
            lines(text.as_bytes()).map(|l| std::str::from_utf8(trim(l)).unwrap()).collect();
        assert_eq!(actual, expected.iter().map(|l| l.trim()).collect::<Vec<_>>());
    }
}

#[test]
fn trims_unicode_whitespace_like_str_trim() {
    for text in ["\u{3000}x\u{a0}", "\u{85}\u{2028}", "\x0b é \x0c", "x\u{200b}", "日本"] {
        assert_eq!(trim(text.as_bytes()), text.trim().as_bytes(), "{text:?}");
    }
}

#[test]
fn counts_text_that_is_not_utf8() {
    let content = b"// caf\xe9\n\xff\xfe\n\n";
    assert_eq!(count_lines(content, "c"), LineCounts { blank: 1, comment: 1, code: 1 });
}

#[test]
fn line_prefixes_match_the_trimmed_line() {
    assert_eq!(count_lines("REM   \n", "bat"), LineCounts { blank: 0, comment: 0, code: 1 });
}

#[test]
fn block_comment_opened_after_code() {
    let content = "mov r0, r1 /* start\n   middle\n   end */\nmov r1, r2\n";
    assert_eq!(count_lines(content, "c"), LineCounts { blank: 0, comment: 2, code: 2 });
}

#[test]
fn block_comment_opener_inside_a_string() {
    let content = "s = \"/* not \\\" a comment\";\nint y;\n";
    assert_eq!(count_lines(content, "c"), LineCounts { blank: 0, comment: 0, code: 2 });
    let content = "c = '\"'; d = '\\''; /* open\n*/\n";
    assert_eq!(count_lines(content, "c"), LineCounts { blank: 0, comment: 1, code: 1 });
}

#[test]
fn block_comment_closed_mid_line_before_code() {
    let content = "a /* x */ b;\nint y;\n";
    assert_eq!(count_lines(content, "c"), LineCounts { blank: 0, comment: 0, code: 2 });
}

#[test]
fn block_comment_reopened_after_code() {
    let content = "a /* x /* y */ b;\nc;\nd /* x */ e /* y\n z */\n/* p */ q /* r\n*/\n";
    assert_eq!(count_lines(content, "c"), LineCounts { blank: 0, comment: 2, code: 4 });
    let content = "/* a\n b */ c; /* d\n e */\n";
    assert_eq!(count_lines(content, "c"), LineCounts { blank: 0, comment: 2, code: 1 });
}

#[test]
fn html_comment_opener_after_text_is_text() {
    let content = "```\n<script></script><!--\n```\ntext\n";
    assert_eq!(count_lines(content, "md"), LineCounts { blank: 0, comment: 0, code: 4 });
}

#[test]
fn block_comment_opener_after_a_line_comment() {
    let content = "a; // x /* y\nb;\n";
    assert_eq!(count_lines(content, "rs"), LineCounts { blank: 0, comment: 0, code: 2 });
}

#[test]
fn rust_lifetimes_are_not_quotes() {
    let content = "fn f<'a>(x: &'a str) {} /* c\n*/\n";
    assert_eq!(count_lines(content, "rs"), LineCounts { blank: 0, comment: 1, code: 1 });
}

#[test]
fn rust_raw_strings_hide_quotes_and_comment_openers() {
    let content = "let r = r#\"a \" /* b\"#;\nlet y = 2;\n";
    assert_eq!(count_lines(content, "rs"), LineCounts { blank: 0, comment: 0, code: 2 });
    let content = "let r = r\"/*\"; let b = br##\"\"# /*\"##; c\"x /* y\";\nz;\n";
    assert_eq!(count_lines(content, "rs"), LineCounts { blank: 0, comment: 0, code: 2 });
    // A raw string ends only at its own number of hashes, after which a comment can open.
    let content = "let r = r#\"\"\"#; /* c\n*/\n";
    assert_eq!(count_lines(content, "rs"), LineCounts { blank: 0, comment: 1, code: 1 });
    // An identifier that ends in `r` does not start a raw string, nor does a raw identifier.
    let content = "let s = bar\"x\" /* c\n*/\nlet r#type = 1; /* d\n*/\n";
    assert_eq!(count_lines(content, "rs"), LineCounts { blank: 0, comment: 2, code: 2 });
}

/// The blank, comment and code counts of `content` in the language of `ext`.
fn counts(content: &str, ext: &str) -> (usize, usize, usize) {
    let LineCounts { blank, comment, code } = count_lines(content, ext);
    (blank, comment, code)
}

#[test]
fn rust_strings_span_lines() {
    let content = "let s = \"a\n// in\n/* in\n\";\n// c\n";
    assert_eq!(counts(content, "rs"), (0, 1, 4));
    // An escaped quote does not end a string, nor does a backslash before a line break.
    let content = "let s = \"a \\\" // b \\\n    // in\n    \";\n// c\n";
    assert_eq!(counts(content, "rs"), (0, 1, 3));
    // An escaped backslash does not escape the quote after it.
    let content = "let s = \"\\\\\";\n// c\n";
    assert_eq!(counts(content, "rs"), (0, 1, 1));
}

#[test]
fn rust_raw_strings_span_lines() {
    let content = "let r = r##\"a \"# // b\n/* in\n\"#\n\"##; // c\n// d\n";
    assert_eq!(counts(content, "rs"), (0, 1, 4));
    let content = "let b = br#\"\n// in\n\"#;\nlet c = cr\"\n// in\n\";\n// d\n";
    assert_eq!(counts(content, "rs"), (0, 1, 6));
    // A raw string has no escapes, so a backslash before its end does not hide it.
    let content = "let r = r\"\\\";\n// c\n";
    assert_eq!(counts(content, "rs"), (0, 1, 1));
}

#[test]
fn rust_characters_and_lifetimes_do_not_open_strings() {
    let content = "let a = '\"'; let b = b'\"'; let c = '\\'';\n// c\n";
    assert_eq!(counts(content, "rs"), (0, 1, 1));
    let content = "fn f<'a>(x: &'a str) -> &'a str { \"x\" }\n// c\nimpl<'a, 'b> X {}\n// d\n";
    assert_eq!(counts(content, "rs"), (0, 2, 2));
}

#[test]
fn quotes_in_comments_do_not_open_strings() {
    let content = "let x = 1; // don't say \"\n// c\n";
    assert_eq!(counts(content, "rs"), (0, 1, 1));
    let content = "let x = /* \" */ 1;\n// c\nlet y = 2; /* \"\n\" */\n// d\n";
    assert_eq!(counts(content, "rs"), (0, 3, 2));
    let content = "x = 1  # don't\n# c\n";
    assert_eq!(counts(content, "py"), (0, 1, 1));
}

#[test]
fn strings_that_do_not_span_lines_end_with_the_line() {
    let content = "char *s = \"abc /* x\n// c\nint y;\n";
    assert_eq!(counts(content, "c"), (0, 1, 2));
    let content = "x = 'abc\n# c\n";
    assert_eq!(counts(content, "py"), (0, 1, 1));
    let content = "let s = 'abc `\n// c\n";
    assert_eq!(counts(content, "js"), (0, 1, 1));
    let content = "String s = \"abc\n// c\n";
    assert_eq!(counts(content, "java"), (0, 1, 1));
}

#[test]
fn blank_lines_inside_strings_are_blank() {
    assert_eq!(counts("let s = \"a\n\n// in\n\";\n", "rs"), (1, 0, 3));
}

#[test]
fn python_triple_quoted_strings_in_expressions_are_code() {
    let content = "x = rb'''a\n# in\n'''  # c\ny = f\"\"\"{x}\n# in\n\"\"\"\n# d\n";
    assert_eq!(counts(content, "py"), (0, 1, 6));
    // A triple of the other quote, or an escaped quote, does not end the string.
    let content = "x = \"\"\"a ''' \\\"\"\"\n# in\n\"\"\"\n# c\n";
    assert_eq!(counts(content, "py"), (0, 1, 3));
    // An empty string does not start a triple-quoted one, and an empty triple-quoted string
    // ends where it starts.
    let content = "x = '' + \"\"\n# c\ny = \"\"\"\"\"\"\n# d\n";
    assert_eq!(counts(content, "py"), (0, 2, 2));
    // Quotes inside a single-quoted string do not open a triple-quoted one.
    let content = "x = '\"\"\"'\n# c\n";
    assert_eq!(counts(content, "py"), (0, 1, 1));
}

#[test]
fn python_docstrings_are_comments() {
    let content = "'''Module.\n# in\n'''\nimport os\n";
    assert_eq!(counts(content, "py"), (0, 3, 1));
    let content = "class A:\n    \"\"\"Class.\n\n    More.\n    \"\"\"\n\n    x = 1\n";
    assert_eq!(counts(content, "py"), (2, 3, 2));
    let content = "def f():\n    \"\"\"Doc.\n    # in\n    \"\"\"\n    # c\n    return 1\n";
    assert_eq!(counts(content, "py"), (0, 4, 2));
    let content = "async def f(\n    a,\n) -> int:  # c\n    \"\"\"Doc.\n    \"\"\"\n";
    assert_eq!(counts(content, "py"), (0, 2, 3));
}

#[test]
fn python_docstrings_on_one_line_and_with_prefixes() {
    assert_eq!(counts("def f():\n    \"\"\"Doc.\"\"\"\n    pass\n", "py"), (0, 1, 2));
    assert_eq!(counts("def f():\n    'Doc.'\n    pass\n", "py"), (0, 1, 2));
    assert_eq!(counts("x = 1\n\"\"\"Doc.\"\"\"  # c\n", "py"), (0, 1, 1));
    let content = "def f():\n    r\"\"\"Doc \\d.\n    \"\"\"\n    U'''x'''\n    B\"y\"\n";
    assert_eq!(counts(content, "py"), (0, 4, 1));
    // An f-string is not a docstring.
    assert_eq!(counts("def f():\n    f\"\"\"{x}\n    \"\"\"\n", "py"), (0, 0, 3));
}

#[test]
fn python_strings_in_expressions_are_not_docstrings() {
    assert_eq!(counts("x = \"\"\"a\n\"\"\"\n", "py"), (0, 0, 2));
    assert_eq!(counts("f(\"\"\"a\n\"\"\")\n", "py"), (0, 0, 2));
    assert_eq!(counts("\"\"\"a {}\n\"\"\".format(x)\n", "py"), (0, 0, 2));
    assert_eq!(counts("\"%s\" % x\n'a'.upper()\n\"\"\"b\"\"\", c\n", "py"), (0, 0, 3));
    // A string continues a statement after an open bracket, a comma, an operator or a backslash.
    let content = "x = (\n    \"a\"\n    # c\n    \"b\"\n)\ny = 1 + \\\n    'c'\nz = [1,\n'd']\n";
    assert_eq!(counts(content, "py"), (0, 1, 8));
    let content = "x = f(a)(\n\"\"\"b\n\"\"\")\ny = c or\\\n\"d\"\nz = (e if f else\n\"g\")\n";
    assert_eq!(counts(content, "py"), (0, 0, 7));
    let content = "x = (\"a\"\n     \"b\")\ny = {\n    'k': 1}\n'''Doc.'''\n";
    assert_eq!(counts(content, "py"), (0, 1, 4));
    // A string after a line that ends with a string outside brackets starts a statement.
    let content = "X = \"a\"\n\"\"\"Doc of X.\"\"\"\n\"\"\"\nMore.\n\"\"\"\n'''Again.'''\n";
    assert_eq!(counts(content, "py"), (0, 5, 1));
}

#[test]
fn python_docstrings_hold_quotes_and_crlf_line_endings() {
    let content =
        "def f():\n    \"\"\"It's \"quoted\" and ''' too.\n    \\\"\"\" still\n    \"\"\"\n";
    assert_eq!(counts(content, "py"), (0, 3, 1));
    let content = "def f():\r\n    \"\"\"Doc.\r\n\r\n    \"\"\"\r\n    return 1\r\n";
    assert_eq!(counts(content, "py"), (1, 2, 2));
    assert_eq!(counts("x = 1\r\n'''Doc.'''\r\n", "py"), (0, 1, 1));
}

#[test]
fn docstrings_are_comments_only_in_languages_with_python_syntax() {
    let content = "\"\"\"Doc.\n\"\"\"\nx = 1\n";
    for key in ["pyi", "pyw", "pyx", "pxd", "pxi", "bzl", "bazel", "bzlmod", "star", "scons"] {
        assert_eq!(counts(content, key), (0, 2, 1), "{key}");
    }
    for key in ["dart", "groovy", "graphql", "kt", "jl", "ex", "toml"] {
        assert_eq!(counts(content, key), (0, 0, 3), "{key}");
    }
}

#[test]
fn javascript_template_literals_span_lines() {
    let content = "const a = `x\n// in\n/* in\n`;\n// c\n";
    assert_eq!(counts(content, "js"), (0, 1, 4));
    // An escaped backtick, and one inside another kind of string, does not end a template.
    let content = "const a = `\\` '\n// in\n`; const b = \"`\" + '`';\n// c\n";
    assert_eq!(counts(content, "ts"), (0, 1, 3));
    // A substitution holds code, whose strings and braces are skipped.
    let content = "const a = `${f({ b: '}' }, `${`}`}`)}\n// in\n`;\n// c\n";
    assert_eq!(counts(content, "tsx"), (0, 1, 3));
    // A substitution and a template inside it can span lines.
    let content = "const a = `${c ? `\n// in\n` : \"\"}\n// in\n`;\n// c\n";
    assert_eq!(counts(content, "js"), (0, 1, 5));
    let content = "const a = `${\n  // c\n  x\n}\n// in\n`;\n";
    assert_eq!(counts(content, "js"), (0, 1, 5));
}

#[test]
fn javascript_regular_expressions_hide_quotes() {
    let content = "const f = /^(```|'|\\/)[/`]/.test(x);\n// c\nif (a) return /`/g;\n// d\n";
    assert_eq!(counts(content, "ts"), (0, 2, 2));
    // Division is not a regular expression, nor is the end of a JSX tag.
    let content = "const a = b / 2 + `\n// in\n` / c;\n// c\nconst d = <a href={`/`}></a>;\n// e\n";
    assert_eq!(counts(content, "jsx"), (0, 2, 4));
}

#[test]
fn javascript_single_quotes_hide_comment_openers() {
    let content = "const a = '/*'; const b = `\n`;\nconst c = 1;\n";
    assert_eq!(counts(content, "js"), (0, 0, 3));
}

#[test]
fn go_raw_strings_span_lines() {
    let content = "var s = `a \\\n// in\n/* in\n`\n// c\n";
    assert_eq!(counts(content, "go"), (0, 1, 4));
    let content = "var r = '`'\nvar t = \"`\"\n// c\n";
    assert_eq!(counts(content, "go"), (0, 1, 2));
}

#[test]
fn cpp_raw_strings_span_lines() {
    let content = "auto s = R\"x(a\n// in\n)\" /* )y\" in\n)x\";\n// c\n";
    assert_eq!(counts(content, "cpp"), (0, 1, 4));
    let content = "auto s = u8R\"(\n// in\n)\";\n// c\n";
    assert_eq!(counts(content, "hpp"), (0, 1, 3));
    // An identifier that ends in `R` does not start a raw string.
    let content = "auto s = BAR\"(\";\n// c\n";
    assert_eq!(counts(content, "cc"), (0, 1, 1));
}

#[test]
fn csharp_verbatim_and_raw_strings_span_lines() {
    let content = "var v = @\"a \"\"\n// in \"\"\n\"; // c\n// d\n";
    assert_eq!(counts(content, "cs"), (0, 1, 3));
    let content = "var q = $@\"\n// in\n\";\nvar r = @$\"\n// in\n\";\n// c\n";
    assert_eq!(counts(content, "cs"), (0, 1, 6));
    let content = "var r = \"\"\"\"\n  // in \"\"\"\n  \"\"\"\";\n// c\n";
    assert_eq!(counts(content, "cs"), (0, 1, 3));
    let content = "var e = \"\" + \"\\\"\";\n// c\n";
    assert_eq!(counts(content, "cs"), (0, 1, 1));
}

#[test]
fn text_blocks_span_lines() {
    let content = "String s = \"\"\"\n    // in \\\"\"\"\n    \"\"\";\n// c\n";
    assert_eq!(counts(content, "java"), (0, 1, 3));
    for ext in ["kt", "kts", "scala"] {
        // These strings have no escapes, and a run of quotes ends one after the whole run.
        let content = "val s = \"\"\"\n// in \\\"\"\"\n// c\nval t = \"\"\"a\"\"\"\"\n// d\n";
        assert_eq!(counts(content, ext), (0, 2, 3), "{ext}");
    }
    let content =
        "let s = \"\"\"\n// in\n\"\"\"\nlet r = #\"\"\"\n// \" \"\"\" in\n\"\"\"#\n// c\n";
    assert_eq!(counts(content, "swift"), (0, 1, 6));
    let content = "let r = #\"a\"b\"#\n// c\n";
    assert_eq!(counts(content, "swift"), (0, 1, 1));
}

#[test]
fn toml_multiline_strings() {
    let content = "a = \"\"\"\n# in\n\"\"\"\nb = '''\n# in \\'''\n# c\nc = 'x' # it's\n# d\n";
    assert_eq!(counts(content, "toml"), (0, 2, 6));
}

#[test]
fn languages_without_multiline_strings_are_unchanged() {
    // Strings in C do not span lines, and quotes are not scanned in shell scripts.
    assert_eq!(counts("char *s = \"\\\n// c\n", "c"), (0, 1, 1));
    assert_eq!(counts("echo \"\n# c\n\"\n", "sh"), (0, 1, 2));
}

#[test]
fn block_comment_opens_after_plain_lines() {
    let content = "int a;\n// c\n\n  /* open\n   inside */\nint b; /* x */\n/* one */\nint c;\n";
    assert_eq!(counts(content, "c"), (1, 4, 3));
    // The block delimiter `--[[` starts with the line delimiter `--`.
    let content = "local a = 1\n-- c\n--[[ open\ninside ]]\nlocal b = 2\n";
    assert_eq!(counts(content, "lua"), (0, 3, 2));
}

#[test]
fn plain_lines_with_crlf_endings() {
    let content = "int a;\r\n// c\r\n\r\n \t\r\n/* open\r\n */\r\nint b;\r\n";
    assert_eq!(counts(content, "c"), (2, 3, 2));
    // `REM ` ends in a space, so the line is matched without its trailing whitespace.
    let content = "x = 1\r\nREM \r\nREM c\r\n";
    assert_eq!(counts(content, "bat"), (0, 1, 2));
}

#[test]
fn plain_lines_start_after_whitespace_outside_ascii() {
    let content = "\u{3000}// c\n\u{a0}int a;\n\u{3000}\n\u{e9}t\u{e9};\n";
    assert_eq!(counts(content, "c"), (1, 1, 2));
}

#[test]
fn leading_byte_order_mark_is_ignored() {
    let content = b"\xEF\xBB\xBF// bom\nint x;\n";
    assert_eq!(count_lines(content, "c"), LineCounts { blank: 0, comment: 1, code: 1 });
    assert_eq!(count_lines(b"\xEF\xBB\xBF\n", "c"), LineCounts { blank: 1, comment: 0, code: 0 });
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

#[yare::parameterized(
    env = { "#!/usr/bin/env python3\nprint(1)\n", Some("py") },
    versioned = { "#!/usr/bin/python3.12\n", Some("py") },
    env_options = { "#!/usr/bin/env -S LANG=C node --flag\n", Some("js") },
    direct = { "#!/bin/sh\n", Some("sh") },
    space_after_bang = { "#! /bin/bash -e\n", Some("bash") },
    crlf = { "#!/usr/bin/env ruby\r\n", Some("rb") },
    unknown = { "#!/usr/bin/env tclsh\n", None },
    not_first_line = { "\n#!/bin/sh\n", None },
    no_shebang = { "MIT License\n", None },
    empty = { "", None },
)]
fn shebang_keys(content: &str, expected: Option<&str>) {
    assert_eq!(shebang_key(content.as_bytes()), expected);
}

#[test]
fn assembly_comments_depend_on_the_dialect() {
    let gnu = "#include <asm.h>\n// line\n/* block\n */\nmov x0, x1\n";
    assert_eq!(count_lines(gnu, "s"), LineCounts { blank: 0, comment: 3, code: 2 });
    assert_eq!(
        count_lines("; comment\nmov eax, 1\n", "asm"),
        LineCounts { blank: 0, comment: 1, code: 1 }
    );
    assert_eq!(language_name("s"), language_name("asm"));
}

#[test]
fn erb_comments() {
    let content = "<%# one line %>\n<%# two\n lines %>\n<!-- html -->\n<%= x %>\n";
    assert_eq!(count_lines(content, "erb"), LineCounts { blank: 0, comment: 4, code: 1 });
}

#[test]
fn m4_and_autoconf_comments() {
    let content = "dnl a comment\n# another\nAC_INIT([x], [1])\n";
    for key in ["m4", "autoconf"] {
        assert_eq!(count_lines(content, key), LineCounts { blank: 0, comment: 2, code: 1 });
    }
}

#[test]
fn tex_linker_script_and_device_tree_comments() {
    assert_eq!(
        count_lines("% comment\n\\section{A}\n", "tex"),
        LineCounts { blank: 0, comment: 1, code: 1 }
    );
    assert_eq!(
        count_lines("/* comment */\nSECTIONS {\n}\n", "ld"),
        LineCounts { blank: 0, comment: 1, code: 2 }
    );
    assert_eq!(
        count_lines("// comment\n/dts-v1/;\n", "dts"),
        LineCounts { blank: 0, comment: 1, code: 1 }
    );
}

#[test]
fn new_file_keys_and_languages() {
    use std::path::Path;
    let name = |path: &str| language_name(&file_key(Path::new(path))).to_string();
    assert_eq!(name("arch/x86/entry.S"), "Assembly");
    assert_eq!(name("boot.asm"), "Assembly");
    assert_eq!(name("a/b.dtsi"), "Device Tree");
    assert_eq!(name("doc/index.rst"), "reStructuredText");
    assert_eq!(name("app/views/a.html.erb"), "ERB");
    for ruby in ["Rakefile", "Gemfile", "lib/tasks/a.rake", "x.gemspec"] {
        assert_eq!(name(ruby), "Ruby", "{ruby}");
    }
    assert_eq!(name("m4/foo.m4"), "M4");
    assert_eq!(name("configure.ac"), "Autoconf");
    assert_eq!(name("configure.in"), "Autoconf");
    assert_eq!(name("src/Makefile.am"), "Automake");
    assert_eq!(name("x.awk"), "AWK");
    assert_eq!(name("scan.l"), "Lex");
    assert_eq!(name("parse.yy"), "Yacc");
    assert_eq!(name("vmlinux.lds"), "Linker Script");
    assert_eq!(name("paper.sty"), "TeX");
    assert!(!is_counted("po"));
}

#[test]
fn documentation_data_and_build_configuration_are_not_split() {
    for key in ["rst", "tex", "sty", "m4", "autoconf", "automake", "ld", "lds", "json", "yaml"] {
        assert!(!is_split(key), "{key}");
    }
    for key in ["s", "asm", "dts", "erb", "awk", "l", "y", "rake"] {
        assert!(is_split(key), "{key}");
    }
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

#[test]
fn functional_language_comments_nest() {
    let content = "{- a {- nested -} still\n-}\n-- line\nmain = putStrLn \"{-\" -- c\n";
    assert_eq!(counts(content, "hs"), (0, 3, 1));
    let content = "(* a (* nested *) still\n*)\nlet s = \"(* not\na comment *)\"\n";
    assert_eq!(counts(content, "ml"), (0, 2, 2));
    let content = "/- a /- nested -/ still\n-/\n-- line\ndef x := 1 /- c\n-/\n";
    assert_eq!(counts(content, "lean"), (0, 4, 1));
    let content = "#= a #= nested =# still\n=#\n# line\nx = \"\"\"\n# in\n\"\"\"\n";
    assert_eq!(counts(content, "jl"), (0, 3, 3));
    let content = "/* a /* nested */ still\n*/\n/// doc\nvoid main() {}\n";
    assert_eq!(counts(content, "dart"), (0, 3, 1));
}

#[test]
fn a_block_comment_opens_after_code_in_ml_languages() {
    let content = "let x = 1 (* starts\n   here *)\nlet y = 2\n";
    assert_eq!(counts(content, "ml"), (0, 1, 2));
    let content = "x = 1 {- starts\n   here -}\n";
    assert_eq!(counts(content, "hs"), (0, 1, 1));
}

#[test]
fn heredocs_and_indented_strings_are_code() {
    let content = "@moduledoc \"\"\"\n## Examples\n\"\"\"\n# c\ndef f, do: ?\"\n# d\n";
    assert_eq!(counts(content, "ex"), (0, 2, 4));
    let content = "script = ''\n  # a shell comment\n'';\n# c\n/* d */\n";
    assert_eq!(counts(content, "nix"), (0, 2, 3));
}

#[test]
fn line_comment_languages() {
    assert_eq!(counts("// c\n/// doc\nconst x = 1;\n", "zig"), (0, 2, 1));
    assert_eq!(counts("; c\n;; d\n(defn f [] \"; s\")\n", "clj"), (0, 2, 1));
    assert_eq!(counts("% c\n%% d\n-module(a).\n", "erl"), (0, 2, 1));
    let content = "# c\n// d\n/* e\n*/\nresource \"a\" \"b\" {}\n";
    assert_eq!(counts(content, "tf"), (0, 4, 1));
    assert_eq!(counts("// c\ncontract A { string s = '//'; }\n", "sol"), (0, 1, 1));
    let content = "// c\nplugins { id 'java' }\ndef s = '''\n// in\n'''\n";
    assert_eq!(counts(content, "gradle"), (0, 1, 4));
}

#[test]
fn mdx_has_html_and_jsx_comments() {
    let content = "# Title\n<!-- a -->\n{/* b\n*/}\n<Chart />\n";
    assert_eq!(counts(content, "mdx"), (0, 3, 2));
}

#[test]
fn languages_added_for_their_extensions() {
    use std::path::Path;
    let name = |path: &str| language_name(&file_key(Path::new(path))).to_string();
    for (path, language) in [
        ("build.zig", "Zig"),
        ("build.zig.zon", "Zig"),
        ("src/Main.hs", "Haskell"),
        ("lib/a.ex", "Elixir"),
        ("test/a_test.exs", "Elixir"),
        ("bin/main.ml", "OCaml"),
        ("lib/a.mli", "OCaml"),
        ("lib/main.dart", "Dart"),
        ("src/A.jl", "Julia"),
        ("src/a/core.clj", "Clojure"),
        ("src/a.cljs", "Clojure"),
        ("src/a.erl", "Erlang"),
        ("include/a.hrl", "Erlang"),
        ("flake.nix", "Nix"),
        ("main.tf", "Terraform"),
        ("prod.tfvars", "Terraform"),
        ("Main.lean", "Lean"),
        ("docs/a.mdx", "MDX"),
        ("src/Token.sol", "Solidity"),
        ("app/build.gradle", "Gradle"),
        ("app/build.gradle.kts", "Kotlin"),
    ] {
        assert_eq!(name(path), language, "{path}");
        assert!(is_counted(&file_key(Path::new(path))), "{path}");
    }
    for key in ["mdx", "gradle", "tfvars"] {
        assert!(!is_split(key), "{key}");
    }
    for key in ["zig", "hs", "ex", "ml", "dart", "jl", "clj", "erl", "nix", "tf", "sol"] {
        assert!(is_split(key), "{key}");
    }
    assert_eq!(shebang_key(b"#!/usr/bin/env elixir\n"), Some("ex"));
    assert_eq!(shebang_key(b"#!/usr/bin/env escript\n"), Some("erl"));
    assert_eq!(shebang_key(b"#!/usr/bin/env julia\n"), Some("jl"));
}

#[test]
fn visual_basic_comments_and_strings() {
    let content = "' c\nREM d\nRem e\n''' <summary>\nDim s = \"it's\" ' f\nRemove()\n";
    assert_eq!(counts(content, "vb"), (0, 4, 2));
    // A string spans lines, and a backslash does not escape its closing quote.
    let content = "Dim s = \"a \"\"\n' in\n\"\nDim p = \"C:\\\"\n' c\n";
    assert_eq!(counts(content, "vb"), (0, 1, 4));
}

#[test]
fn asciidoc_comments() {
    let content = "= Title\n// c\n////\nblock\n\n////\ntext // not a comment\n";
    assert_eq!(counts(content, "adoc"), (1, 4, 2));
}

#[test]
fn bazel_comments_and_strings() {
    let content = "# c\nload(\"//a:b.bzl\", \"x\")\ngenrule(cmd = \"\"\"\n# in\n\"\"\")\n";
    for key in ["bzl", "bazel", "bzlmod", "star"] {
        assert_eq!(counts(content, key), (0, 1, 4), "{key}");
    }
}

#[test]
fn puppet_comments_and_strings() {
    let content = "# c\n/* d\n*/\nfile { '/tmp/#/*': ensure => \"/*\" } /* e\n*/\n";
    assert_eq!(counts(content, "pp"), (0, 4, 1));
}

#[test]
fn jinja_comments_span_lines() {
    let content = "{# c #}\n{# one\ntwo #}\n{{ x }} {# e #}\n<p>{% if a %}{# f</p>\n";
    assert_eq!(counts(content, "j2"), (0, 3, 2));
}

#[test]
fn coq_comments_nest_and_strings_double_their_quotes() {
    let content = "(* a (* b *) c\n*)\nLemma x : \"(*\" = \"*)\".\nProof. (* d\n*) Qed.\n";
    assert_eq!(counts(content, "coq"), (0, 2, 3));
    let content = "Definition p := \"C:\\\".\n(* c *)\nDefinition s := \"a \"\"\n(* in *)\n\".\n";
    assert_eq!(counts(content, "coq"), (0, 1, 4));
}

#[test]
fn verilog_comments() {
    let content = "// c\n/* d\n*/\nmodule m; // e\n  assign s = \"/*\";\nendmodule /* f\n*/\n";
    assert_eq!(counts(content, "v"), (0, 4, 3));
}

#[test]
fn system_verilog_comments_and_strings() {
    // A backslash escapes a quote, and a block comment does not nest.
    let content = "// c\nmodule m; string s = \"\\\" /*\"; /* a /* b */\nendmodule\n/* d\n*/\n";
    for key in ["sv", "svh"] {
        assert_eq!(counts(content, key), (0, 3, 2), "{key}");
    }
}

#[yare::parameterized(
    coq_require = { "From Coq Require Import List.\n", "coq" },
    coq_attribute = { "#[local] Instance i : C.\nDefinition x := 1.\n", "coq" },
    coq_after_blank_lines = { "\n  \n\tTheorem t : True.\n", "coq" },
    coq_implicit_arguments = { "Set Implicit Arguments.\n", "coq" },
    coq_module_is_capitalized = { "Module M.\nEnd M.\n", "coq" },
    coq_ssreflect_slashes = { "Lemma l : P.\nProof. by move=> //. Qed.\n", "coq" },
    coq_comment_mentions_verilog = { "(* module m; endmodule *)\nLemma l : True.\n", "coq" },
    verilog_module = { "module top(input clk);\nendmodule\n", "v" },
    verilog_directive = { "`timescale 1ns / 1ps\nmodule m;\n", "v" },
    verilog_attribute = { "(* keep *)\nmodule m(input a, output b);\nassign b = a;\nendmodule\n", "v" },
    verilog_comment_mentions_coq = { "/* Theorem Lemma Proof. Qed. */\nmodule m;\n", "v" },
    verilog_always = { "always @(posedge clk) begin\n  q <= d;\nend\n", "v" },
    verilog_crlf = { "\r\nwire w;\r\n", "v" },
    verilog_string_mentions_coq = { "initial $display(\"Lemma Theorem\");\n", "v" },
    short_coq = { "Qed.", "coq" },
    short_verilog = { "reg", "v" },
    empty = { "", "coq" },
    blank = { "\n\n", "coq" },
    no_signal = { "{\n}\n", "coq" },
    tie = { "End x\nend x\n", "coq" },
)]
fn v_keys(content: &str, expected: &str) {
    assert_eq!(v_key(content.as_bytes()), expected);
}

#[test]
fn v_key_skips_long_license_headers() {
    let verilog = format!("{}`timescale 1ns/1ps\nmodule m;\n", "// SPDX and more\n".repeat(200));
    assert!(verilog.len() > 1024);
    assert_eq!(v_key(verilog.as_bytes()), "v");
    let coq = format!("(*{}*)\nRequire Import List.\n", " module m; wire w;\n".repeat(200));
    assert!(coq.len() > 1024);
    assert_eq!(v_key(coq.as_bytes()), "coq");
}

#[test]
fn v_key_reads_only_the_start_of_a_file() {
    // Tokens after the prefix are not read, so a file without any before it is Coq.
    let content = format!("{}module m;\n", "\n".repeat(V_PREFIX));
    assert_eq!(v_key(content.as_bytes()), "coq");
    let content = format!("{}module m;\n", "\n".repeat(V_PREFIX - 10));
    assert_eq!(v_key(content.as_bytes()), "v");
}

#[test]
fn languages_and_formats_from_other_counters() {
    use std::path::Path;
    let name = |path: &str| language_name(&file_key(Path::new(path))).to_string();
    for (path, language) in [
        ("src/A.vb", "Visual Basic"),
        ("docs/index.adoc", "AsciiDoc"),
        ("docs/a.asciidoc", "AsciiDoc"),
        ("BUILD", "Bazel"),
        ("a/BUILD.bazel", "Bazel"),
        ("WORKSPACE", "Bazel"),
        ("WORKSPACE.bazel", "Bazel"),
        ("WORKSPACE.bzlmod", "Bazel"),
        ("MODULE.bazel", "Bazel"),
        ("tools/defs.bzl", "Bazel"),
        ("Tiltfile.star", "Bazel"),
        ("manifests/init.pp", "Puppet"),
        ("templates/a.conf.j2", "Jinja"),
        ("templates/a.jinja", "Jinja"),
        ("templates/a.jinja2", "Jinja"),
        ("rtl/top.v", "Verilog"),
        ("rtl/defs.vh", "Verilog"),
        ("rtl/core.sv", "SystemVerilog"),
        ("rtl/pkg.svh", "SystemVerilog"),
        ("src/A.csproj", "XML"),
        ("src/A.vbproj", "XML"),
        ("src/A.fsproj", "XML"),
        ("Directory.Build.props", "XML"),
        ("Directory.Build.targets", "XML"),
        ("App.xaml", "XML"),
        ("Resources.resx", "XML"),
        ("A.nuspec", "XML"),
    ] {
        assert_eq!(name(path), language, "{path}");
        assert!(is_counted(&file_key(Path::new(path))), "{path}");
    }
    assert_eq!(language_name("coq"), "Coq");
    assert!(is_counted("coq"));
    for key in ["adoc", "asciidoc", "bzl", "bazel", "bzlmod", "star"] {
        assert!(!is_split(key), "{key}");
    }
    for key in
        ["vb", "pp", "j2", "jinja", "jinja2", "v", "vh", "sv", "svh", "coq", "csproj", "xaml"]
    {
        assert!(is_split(key), "{key}");
    }
    // Linux's tools name their Kbuild makefiles `Build`, which are not Bazel files.
    assert_eq!(file_key(Path::new("tools/objtool/Build")), "");
}

#[test]
fn every_language_in_the_table_is_counted_with_a_comment_syntax() {
    use std::path::Path;
    for language in LANGUAGES {
        let keys = language.names.iter().map(|n| file_key(Path::new(n)));
        let keys =
            keys.chain(language.extensions.iter().map(|e| file_key(Path::new(&format!("a.{e}")))));
        for key in keys {
            assert!(is_counted(&key), "{key}");
            assert!(comment_style(&key).is_some(), "{key}");
            assert_eq!(language_name(&key), language.name, "{key}");
            assert_eq!(is_split(&key), language.split, "{key}");
        }
    }
}

#[test]
fn extensions_added_to_languages_that_were_counted() {
    use std::path::Path;
    let name = |path: &str| language_name(&file_key(Path::new(path))).to_string();
    for (path, language) in [
        ("a.hxx", "C++"),
        ("a.h++", "C++"),
        ("a.c++", "C++"),
        ("a.inl", "C++"),
        ("a.ipp", "C++"),
        ("a.tpp", "C++"),
        ("a.cppm", "C++"),
        ("a.ccm", "C++"),
        ("a.mxx", "C++"),
        ("a.ixx", "C++"),
        ("a.pyi", "Python"),
        ("a.pyw", "Python"),
        ("a.psm1", "PowerShell"),
        ("a.psd1", "PowerShell"),
        ("a.csx", "C#"),
        ("a.sc", "Scala"),
        ("terragrunt.hcl", "Terraform"),
        ("a.luau", "Lua"),
        ("A.vcxproj", "XML"),
        ("A.vcxproj.filters", "XML"),
        ("A.Format.ps1xml", "XML"),
    ] {
        assert_eq!(name(path), language, "{path}");
    }
    assert!(!is_split("hcl") && is_split("tf"));
    assert!(!is_counted(&file_key(Path::new("a.filters"))));
    let content = "#pragma once\nauto s = R\"(\n// in\n)\";\n// c\n";
    assert_eq!(counts(content, "hxx"), (0, 1, 4));
    assert_eq!(counts("<# a\n#>\n# b\nfunction F {}\n", "psm1"), (0, 3, 1));
}

#[test]
fn names_select_languages() {
    use std::path::Path;
    let name = |path: &str| language_name(&file_key(Path::new(path))).to_string();
    for (path, language) in [
        ("ci/Jenkinsfile", "Groovy"),
        ("SConstruct", "SCons"),
        ("src/SConscript", "SCons"),
        ("justfile", "Just"),
        ("Justfile", "Just"),
        (".justfile", "Just"),
        ("a.just", "Just"),
        ("home/.bashrc", "Shell"),
        (".bash_profile", "Shell"),
        (".zshrc", "Shell"),
        (".zprofile", "Shell"),
        (".zshenv", "Shell"),
        (".profile", "Shell"),
        ("dotfiles/bashrc", "Shell"),
        ("dotfiles/bash_profile", "Shell"),
        ("dotfiles/zshrc", "Shell"),
        ("dotfiles/zprofile", "Shell"),
        ("dotfiles/zshenv", "Shell"),
        ("BUILD.bazel", "Bazel"),
        ("MODULE.bazel", "Bazel"),
    ] {
        assert_eq!(name(path), language, "{path}");
    }
    // A `profile` without the dot is often another program's configuration.
    assert_eq!(file_key(Path::new("fcitx5/profile")), "");
    assert!(!is_split("scons") && !is_split("just"));
}

#[test]
fn fsharp_comments_nest_and_strings_span_lines() {
    let content = "// c\n(* a (* nested *) still\n*)\nlet s = \"\"\"\n// in\n\"\"\"\n\
                       let v = @\"C:\\\" // x\nlet t = \"a\n(* in *)\n\"\n";
    for key in ["fs", "fsi", "fsx"] {
        assert_eq!(counts(content, key), (0, 3, 7), "{key}");
    }
}

#[test]
fn fortran_comments_in_free_and_fixed_form() {
    let content = "! c\nprogram p\n  x = 1 ! y\n  ! z\nend\n";
    for key in ["f90", "f95", "f03", "f08"] {
        assert_eq!(counts(content, key), (0, 2, 3), "{key}");
    }
    // In fixed form, a `C` in the first column marks a comment, and one in the seventh does not.
    let content = "C comment\nc lower\n* star\n      CALL FOO\n     1  BAR\n! bang\n\
                       \n      X = 1\n      C = 2\n";
    for key in ["f", "for", "f77"] {
        assert_eq!(counts(content, key), (1, 4, 4), "{key}");
    }
    assert_eq!(counts("C = 1\n", "f90"), (0, 0, 1));
}

#[test]
fn cobol_comments_in_the_indicator_column_and_floating() {
    let content = "000100* comment\n000200/ page\n       IDENTIFICATION DIVISION.\n\
                       \x20     *> floating\n       PROGRAM-ID. P. *> e\n";
    for key in ["cbl", "cob", "cobol", "cpy"] {
        assert_eq!(counts(content, key), (0, 3, 2), "{key}");
    }
    assert_eq!(counts("*> c\nDISPLAY 'x'.\n", "cbl"), (0, 1, 1));
}

#[test]
fn c_like_languages_added_for_their_extensions() {
    let content = "// c\n/* d\n*/\nvoid main() { s = \"/*\"; }\n";
    for key in
        ["cu", "cuh", "glsl", "vert", "frag", "geom", "comp", "tesc", "tese", "hlsl", "fx", "metal"]
    {
        assert_eq!(counts(content, key), (0, 3, 1), "{key}");
    }
    let content = "// c\n__global__ void k() { auto s = R\"(\n// in\n)\"; }\n";
    assert_eq!(counts(content, "cu"), (0, 1, 3));
    assert_eq!(
        counts("// c\n/* a /* nested */ still\n*/\ns := `raw\n// in\n`\n", "odin"),
        (0, 3, 3)
    );
    assert_eq!(counts("// c\n/// doc\npub fn f() { \"a\n// in\n\" }\n", "gleam"), (0, 2, 3));
    let content = "# c\n// d\n/* e\n*/\nstruct A { 1: string s = \"//\" }\n";
    assert_eq!(counts(content, "thrift"), (0, 4, 1));
    assert_eq!(counts("// c\n/* d\n*/\n= Heading\n#let x = 1\n", "typ"), (0, 3, 2));
}

#[test]
fn python_like_languages_added_for_their_extensions() {
    let content = "# c\ncdef int x = 1\ns = \"\"\"\n# in\n\"\"\"\n";
    for key in ["pyx", "pxd", "pxi", "scons", "pyi"] {
        assert_eq!(counts(content, key), (0, 1, 4), "{key}");
    }
    let content = "// c\ndef s = '''\n// in\n'''\n/* d */\n";
    for key in ["groovy", "gvy", "gy", "gsh"] {
        assert_eq!(counts(content, key), (0, 2, 3), "{key}");
    }
    let content = "# c\n\"\"\"\n# in a description\n\"\"\"\ntype Query { a: String # d\n}\n";
    assert_eq!(counts(content, "graphql"), (0, 1, 5));
    let content = "# c\nbuild:\n    cargo build # d\nx := '''\n# in\n'''\n";
    assert_eq!(counts(content, "just"), (0, 1, 5));
    assert_eq!(counts("# c\nset s \"a\n# in\n\"\nputs $s ;# d\n", "tcl"), (0, 1, 4));
    assert_eq!(counts("# c\nputs \"x\" # d\n", "cr"), (0, 1, 1));
    let content = "\nMicrosoft Visual Studio Solution File, Format Version 12.00\n\
                       # Visual Studio Version 17\nProject(\"{A}\") = \"A\"\nEndProject\n";
    assert_eq!(counts(content, "sln"), (1, 1, 3));
}

#[test]
fn pascal_comments_and_doubled_quotes() {
    let content = "// c\n{ block\n}\n(* other\n*)\ns := '{'' not a comment';\nx := 1; { e\n}\n";
    for key in ["pas", "dpr", "lpr"] {
        assert_eq!(counts(content, key), (0, 6, 2), "{key}");
    }
}

#[test]
fn d_comments_and_strings() {
    let content = "// c\n/+ a /+ nested +/ still\n+/\n/* d */\nauto s = r\"C:\\\" ~ `\n// in\n`;\n";
    for key in ["d", "di"] {
        assert_eq!(counts(content, key), (0, 4, 3), "{key}");
    }
}

#[yare::parameterized(
    gcc = { "foo.o: foo.c foo.h \\\n  bar.h\n", true },
    several_targets = { "foo.o foo.d: foo.c\n", true },
    rustc = { "/home/u/target/debug/deps/a-1f.d: src/lib.rs\n", true },
    no_extension = { "target/debug/a: src/main.rs\n", true },
    windows_drive = { "C:\\x\\foo.o: foo.c\n", true },
    no_dependencies = { "foo.o:\n", true },
    module = { "module std.stdio;\n", false },
    line_comment = { "// Written in D\nmodule a;\n", false },
    block_comment = { "/**\n * Docs: here\n */\n", false },
    nested_comment = { "/+ a.b: c +/\n", false },
    label = { "private:\n", false },
    version = { "version (Windows):\n", false },
    selective_import = { "import std.stdio : writeln;\n", false },
    selective_import_without_space = { "import std.stdio: writeln,\n", false },
    shebang = { "#!/usr/bin/env rdmd\nvoid main() {}\n", false },
    attribute = { "@safe:\n", false },
    empty = { "", false },
)]
fn make_dependencies(content: &str, expected: bool) {
    assert_eq!(is_make_dependencies(content.as_bytes()), expected);
}

#[test]
fn make_dependencies_are_found_in_a_short_prefix() {
    let content = format!("{}: a.c\n", "a/".repeat(D_PREFIX));
    assert!(!is_make_dependencies(content.as_bytes()));
}

#[test]
fn lisp_comments_nest_and_characters_hide_quotes() {
    let content = "; c\n;;; d\n(defun f () \"Doc\n; in\n\")\n(list ?\" ?\\\" 1)\n; e\n";
    assert_eq!(counts(content, "el"), (0, 3, 4));
    let content = "; c\n#| a #| nested |# still\n|#\n(display \"#| not\")\n(char=? c #\\\")\n; d\n";
    for key in ["scm", "ss", "sld", "sls", "rkt", "lisp", "lsp", "asd"] {
        assert_eq!(counts(content, key), (0, 4, 2), "{key}");
    }
}

#[test]
fn dash_comment_languages_added_for_their_extensions() {
    let content = "-- c\nprocedure P is\n   S : String := \"--\";\nbegin null; end P; -- d\n";
    for key in ["adb", "ads", "ada"] {
        assert_eq!(counts(content, key), (0, 1, 3), "{key}");
    }
    let content = "-- c\n{- a {- nested -} still\n-}\ns = \"\"\"\n-- in\n\"\"\"\n";
    for key in ["elm", "purs"] {
        assert_eq!(counts(content, key), (0, 3, 3), "{key}");
    }
    let content = "-- c\n/* a\n*/\nsignal s : string := \"--\"; -- d\nx <= '0';\n";
    for key in ["vhd", "vhdl"] {
        assert_eq!(counts(content, key), (0, 3, 2), "{key}");
    }
}

#[test]
fn template_comments() {
    let content = "{{! c }}\n{{!-- a }} b\n--}}\n<p>{{name}}</p>\n<!-- d -->\n";
    for key in ["hbs", "handlebars"] {
        assert_eq!(counts(content, key), (0, 4, 1), "{key}");
    }
    assert_eq!(counts("{{! one\ntwo }}\n<p>{{x}}</p>\n", "mustache"), (0, 2, 1));
    for key in ["pug", "jade"] {
        assert_eq!(counts("//- hidden\n// shown\ndiv.a\n  p text\n", key), (0, 2, 2), "{key}");
    }
}

#[test]
fn coffeescript_and_nim_block_comments() {
    let content = "# c\n###\nblock\n###\ns = \"\"\"\n# in\n\"\"\"\nx = 1 # d\n";
    assert_eq!(counts(content, "coffee"), (0, 4, 4));
    let content = "# c\n#[ a #[ nested ]# still\n]#\n##[ doc\n]##\ns = \"\"\"\n# in\n\"\"\"\n";
    for key in ["nim", "nims", "nimble"] {
        assert_eq!(counts(content, key), (0, 5, 3), "{key}");
    }
}

#[test]
fn languages_and_extensions_added_with_prolog_and_notebooks() {
    use std::path::Path;
    let name = |path: &str| language_name(&file_key(Path::new(path))).to_string();
    for (path, language) in [
        ("rules.mk", "Makefile"),
        ("win/Makefile.mak", "Makefile"),
        ("bin/a.ksh", "Shell"),
        ("bin/a.csh", "Shell"),
        ("bin/a.tcsh", "Shell"),
        ("tsconfig.jsonc", "JSONC"),
        ("config.json5", "JSON5"),
        ("package.json", "JSON"),
        ("a.styl", "Stylus"),
        ("meson.build", "Meson"),
        ("meson_options.txt", "Meson"),
        ("meson.options", "Meson"),
        ("plugin/a.vim", "Vim Script"),
        ("vimrc", "Vim Script"),
        (".vimrc", "Vim Script"),
        ("_vimrc", "Vim Script"),
        ("gvimrc", "Vim Script"),
        (".gvimrc", "Vim Script"),
        ("_gvimrc", "Vim Script"),
        ("schema.prisma", "Prisma"),
        ("a.nu", "Nushell"),
        ("Main.qml", "QML"),
        ("Main.hx", "Haxe"),
        ("a.jsonnet", "Jsonnet"),
        ("a.libsonnet", "Jsonnet"),
        ("a.fth", "Forth"),
        ("a.4th", "Forth"),
        ("a.forth", "Forth"),
        ("a.fs", "F#"),
        ("a.f", "Fortran"),
        ("a.st", "Smalltalk"),
        ("Pkg/Foo.class.st", "Smalltalk"),
        ("Pkg/Object.extension.st", "Smalltalk"),
        ("a.ipynb", "Jupyter Notebook"),
        ("Default.aspx", "ASP.NET"),
        ("Site.master", "ASP.NET"),
        ("Global.asax", "ASP.NET"),
        ("Views/Index.cshtml", "Razor"),
        ("Views/Index.vbhtml", "Razor"),
        ("Pages/Home.razor", "Razor"),
        ("default.asp", "ASP"),
        ("icon.svg", "SVG"),
        ("Main.as", "ActionScript"),
        ("index.cfm", "ColdFusion"),
        ("index.cfml", "ColdFusion"),
        ("User.cfc", "ColdFusion CFScript"),
        ("base.html.twig", "Twig"),
    ] {
        assert_eq!(name(path), language, "{path}");
        assert!(is_counted(&file_key(Path::new(path))), "{path}");
    }
    // A `.pl` file is Perl until its content says otherwise, and `.pro` is not counted.
    assert_eq!(name("a.pl"), "Perl");
    assert_eq!(language_name("prolog"), "Prolog");
    assert!(!is_counted("pro"));
    assert!(!is_counted(&file_key(Path::new("data/macros.meson"))));
    for key in ["jsonc", "json5", "meson.build", "jsonnet", "libsonnet", "svg", "mk"] {
        assert!(!is_split(key), "{key}");
    }
    for key in ["styl", "vim", "nu", "qml", "hx", "fth", "st", "prolog", "ipynb", "cshtml", "as"] {
        assert!(is_split(key), "{key}");
    }
}

#[test]
fn json_with_comments_and_strict_json() {
    let content = "// c\n{\n  /* d */\n  \"a\": \"//\", /* e\n  */\n}\n";
    assert_eq!(counts(content, "jsonc"), (0, 3, 3));
    assert_eq!(counts(content, "json"), (0, 0, 6));
    let content = "// c\n{ a: '/* x', b: \"y\",\n/* d */\n}\n";
    assert_eq!(counts(content, "json5"), (0, 2, 2));
}

#[test]
fn hash_comment_languages_added_for_their_extensions_and_names() {
    let content = "# c\nall:\n\techo hi # d\n";
    for key in ["mk", "mak", "ksh", "csh", "tcsh", "nu"] {
        assert_eq!(counts(content, key), (0, 1, 2), "{key}");
    }
    // A Meson multiline string holds no comments, and has no escapes.
    let content = "# c\nproject('x')\ns = '''\n# in \\'''\n# d\n";
    assert_eq!(counts(content, "meson.build"), (0, 2, 3));
    // A Nushell string spans lines.
    assert_eq!(counts("# c\nlet s = \"a\n# in\n\"\n", "nu"), (0, 1, 3));
}

#[test]
fn vim_script_quote_comments_and_vim9_hash_comments() {
    // A `"` that starts a line is a comment, and a `"` after code starts a string or a
    // trailing comment on a line of code.
    let content = "\" c\n  \" d\nlet s = \"a\" \" e\necho 'it''s'\n";
    assert_eq!(counts(content, "vim"), (0, 2, 2));
    let content = "vim9script\n# c\ndef F(): string\n  # d\n  return \"#\"\nenddef\n";
    assert_eq!(counts(content, "vim"), (0, 2, 4));
}

#[test]
fn c_comment_languages_added_for_their_extensions() {
    let content = "// c\n.a\n  /* d */\n  b: url('//x') /* e\n*/\n";
    assert_eq!(counts(content, "styl"), (0, 3, 2));
    let content = "// c\nItem {\n  /* d */\n  property string s: \"/*\"\n  x: /\\/*/.test(s)\n}\n";
    assert_eq!(counts(content, "qml"), (0, 2, 4));
    // Haxe's strings span lines with either quote.
    let content = "// c\nvar s = 'a\n// in\n';\n/* d */\n";
    assert_eq!(counts(content, "hx"), (0, 2, 3));
    let content = "// c\nvar s:String = \"/*\";\n/* d\n*/\n";
    assert_eq!(counts(content, "as"), (0, 3, 1));
    let content = "/// doc\n// c\nmodel A {\n  id Int @id // e\n  s String @default(\"//\")\n}\n";
    assert_eq!(counts(content, "prisma"), (0, 2, 4));
}

#[test]
fn jsonnet_comments_and_text_blocks() {
    let content =
        "// c\n# d\n/* e */\n{\n  s: |||\n    # in\n    // in\n  |||,\n  v: @'a''#',\n}\n";
    for key in ["jsonnet", "libsonnet"] {
        assert_eq!(counts(content, key), (0, 3, 7), "{key}");
    }
}

#[test]
fn forth_comments() {
    let content = "\\ c\n\\G doc\n( stack\n  comment )\n: sq ( n -- n ) dup * ;\n( a ) 1 .\n";
    for key in ["fth", "4th", "forth"] {
        assert_eq!(counts(content, key), (0, 4, 2), "{key}");
    }
}

#[test]
fn smalltalk_comments_strings_and_characters() {
    let content = "\"c\"\nFoo >> bar [\n  \"d\" ^ $\" , 'a \"b\" c'\n  ^ $'\n]\n\"multi\nline\"\n";
    assert_eq!(counts(content, "st"), (0, 3, 4));
    // A string spans lines, and a doubled quote stands for itself in either.
    let content = "s := 'a\n\"in\" ''b''\nc'.\n\"x \"\"y\"\" z\"\n";
    assert_eq!(counts(content, "st"), (0, 1, 3));
}

#[test]
fn prolog_comments_and_quoted_atoms() {
    let content = "% c\n/* d\n*/\nfoo('%', \"/*\").\nbar(X) :- X = 0'%, baz. % e\n";
    assert_eq!(counts(content, "prolog"), (0, 3, 2));
}

#[test]
fn web_template_comments() {
    let content =
        "<%-- c\n--%>\n<%@ Page Language=\"C#\" %>\n<!-- d -->\n<asp:Label runat=\"server\" />\n";
    for key in ["aspx", "ascx", "asmx", "ashx", "asax", "master"] {
        assert_eq!(counts(content, key), (0, 3, 2), "{key}");
    }
    let content = "@* a\nb *@\n@{\n  // c\n  var x = 1; /* d */\n}\n<p>@x</p>\n<!-- e -->\n";
    for key in ["cshtml", "vbhtml", "razor"] {
        assert_eq!(counts(content, key), (0, 4, 4), "{key}");
    }
    let content = "<%\n' c\nResponse.Write \"x\" ' d\n%>\n<!-- e -->\n";
    assert_eq!(counts(content, "asp"), (0, 2, 3));
    assert_eq!(counts("<!-- c -->\n<svg>\n</svg>\n", "svg"), (0, 1, 2));
    let content = "<!--- c\n<!-- d --> --->\n<!-- e -->\n<cfset x = 1>\n";
    for key in ["cfm", "cfml"] {
        assert_eq!(counts(content, key), (0, 3, 1), "{key}");
    }
    assert_eq!(counts("// c\ncomponent {\n/* d */\n}\n", "cfc"), (0, 2, 2));
    assert_eq!(counts("<!--- c --->\n<cfcomponent>\n</cfcomponent>\n", "cfc"), (0, 1, 2));
    assert_eq!(counts("{# c\n#}\n<p>{{ x }}</p>\n", "twig"), (0, 2, 1));
}

#[yare::parameterized(
    prolog_module = { ":- module(lists, [append/3]).\n", "prolog" },
    prolog_use_module = { "/* License\n   my $x; use strict;\n*/\n:- use_module(library(lists)).\n", "prolog" },
    prolog_directive = { ":- dynamic foo/1.\n", "prolog" },
    prolog_facts = { "parent(tom, bob).\nparent(bob, ann).\n", "prolog" },
    prolog_rule = { "grandparent(X, Z) :-\n    parent(X, Y),\n    parent(Y, Z).\n", "prolog" },
    prolog_comments = { "% c\n%% d\n%! e\nfoo.\n", "prolog" },
    prolog_clpfd = { "sum(X, Y) :- X #= Y + 1. % c\n", "prolog" },
    prolog_system_atom = { "'$init'(X) :- X = 1.\n", "prolog" },
    prolog_shebang = { "#!/usr/bin/env swipl\nmain :- true.\n", "prolog" },
    short_prolog = { "foo.", "prolog" },
    perl_strict = { "use strict;\nuse warnings;\n", "pl" },
    perl_shebang = { "#!/usr/bin/perl -w\nfoo.\nbar.\nbaz.\n", "pl" },
    perl_my = { "my $x = 1;\n", "pl" },
    perl_sub = { "sub f {\n    return 1;\n}\n", "pl" },
    perl_print = { "print \"hi\\n\";\n", "pl" },
    perl_hash = { "%h = (a => 1);\n", "pl" },
    perl_pod = { "=head1 NAME\n\nfoo :- bar.\n\n=cut\n1;\n", "pl" },
    perl_comment_and_package = { "# c\npackage Foo;\n1;\n", "pl" },
    perl_data = { "print <DATA>;\n__DATA__\na.\nb.\nc.\n", "pl" },
    short_perl = { "1;", "pl" },
    empty = { "", "pl" },
    blank = { "\n\n", "pl" },
    tie = { "foo.\nbar;\n", "pl" },
)]
fn pl_keys(content: &str, expected: &str) {
    assert_eq!(pl_key(content.as_bytes()), expected);
}

#[test]
fn pl_key_reads_only_the_start_of_a_file() {
    let content = format!("{}foo.\n", "\n".repeat(PL_PREFIX));
    assert_eq!(pl_key(content.as_bytes()), "pl");
    let content = format!("{}foo.\n", "\n".repeat(PL_PREFIX - 10));
    assert_eq!(pl_key(content.as_bytes()), "prolog");
}

#[test]
fn swipl_scripts_are_prolog() {
    assert_eq!(shebang_key(b"#!/usr/bin/env swipl\n"), Some("prolog"));
    assert!(is_counted("prolog"));
}
