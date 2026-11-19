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

impl LineCounts {
    pub fn add(&mut self, other: LineCounts) {
        self.blank += other.blank;
        self.comment += other.comment;
        self.code += other.code;
    }

    /// The counts of the lines that are in `self` but not in `part`, a part of the same text.
    pub fn without(self, part: LineCounts) -> LineCounts {
        LineCounts {
            blank: self.blank.saturating_sub(part.blank),
            comment: self.comment.saturating_sub(part.comment),
            code: self.code.saturating_sub(part.code),
        }
    }
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
        "rake" | "gemspec" | "awk" | "automake" => Some(CommentStyle { line: &["#"], block: &[] }),
        // In GNU assembly, `#` starts a preprocessor directive in `.S` files, and the line
        // comment character differs between architectures, so only C comments are recognized.
        "s" | "dts" | "dtsi" | "l" | "lex" | "y" | "yy" => {
            Some(CommentStyle { line: &["//"], block: &[("/*", "*/")] })
        }
        "asm" => Some(CommentStyle { line: &[";"], block: &[] }),
        "erb" => Some(CommentStyle { line: &[], block: &[("<%#", "%>"), ("<!--", "-->")] }),
        "m4" | "autoconf" => Some(CommentStyle { line: &["#", "dnl"], block: &[] }),
        "ld" | "lds" => Some(CommentStyle { line: &[], block: &[("/*", "*/")] }),
        "tex" | "sty" => Some(CommentStyle { line: &["%"], block: &[] }),
        "rst" => Some(CommentStyle { line: &[], block: &[] }),
        _ => None,
    }
}

/// Count the lines of `content`, using the comment syntax for the extension `ext`.
///
/// A line with code is code, even when a block comment opens or closes on it, and the lines
/// after it inside the comment are comments. Without a known comment syntax, every non-blank line
/// is code. The content need not be UTF-8.
pub fn count_lines(content: &(impl AsRef<[u8]> + ?Sized), ext: &str) -> LineCounts {
    let style = comment_style(ext).unwrap_or(CommentStyle { line: &[], block: &[] });
    let (syntax, content) = (Syntax::new(&style), content.as_ref());
    let opens_after_code =
        if syntax.c_like { syntax.opens_after_code(content) } else { Vec::new() };
    syntax.count(content, &opens_after_code)
}

/// A [`CommentStyle`] with the lookups that counting needs.
struct Syntax<'a> {
    style: &'a CommentStyle,
    /// The first bytes of every comment delimiter, so that most bytes and most lines of code are
    /// told apart by one lookup.
    starts: [bool; 256],
    /// Whether the language has C's `/*` comments, and so C's quotes: strings are `"` with
    /// backslash escapes and characters are `'`. Only in these languages does a block comment
    /// open after code, since the quotes of the others are not known, and a Markdown code block
    /// may show an HTML comment opening.
    c_like: bool,
}

impl<'a> Syntax<'a> {
    fn new(style: &'a CommentStyle) -> Self {
        let c_like = style.block.iter().any(|&(open, _)| open == "/*");
        let mut starts = [false; 256];
        for (open, _) in style.block {
            if let Some(&first) = open.as_bytes().first() {
                starts[usize::from(first)] = true;
            }
        }
        for line in style.line {
            if let Some(&first) = line.as_bytes().first() {
                starts[usize::from(first)] = true;
            }
        }
        Self { style, starts, c_like }
    }

    /// Count the lines of `content`, where a block comment can open after code only on the lines
    /// that start at the sorted offsets `opens_after_code`.
    fn count(&self, content: &[u8], opens_after_code: &[usize]) -> LineCounts {
        let style = self.style;
        let mut counts = LineCounts::default();
        // The close delimiter of the block comment that the current line is inside.
        let mut in_block: Option<&'static [u8]> = None;
        let mut offsets = opens_after_code.iter().copied().peekable();
        let mut may_open = |line: &[u8]| {
            let start = line.as_ptr().addr() - content.as_ptr().addr();
            while offsets.next_if(|&offset| offset < start).is_some() {}
            offsets.peek() == Some(&start)
        };

        for line in lines(content) {
            let trimmed = trim_start(line);
            let Some(&first) = trimmed.first() else {
                counts.blank += 1;
                continue;
            };
            if in_block.is_none() && !self.starts[usize::from(first)] {
                counts.code += 1;
                if !opens_after_code.is_empty() && may_open(line) {
                    in_block = self.open_after_code(trimmed);
                }
                continue;
            }
            // Delimiters such as `REM ` end in a space, so they are matched against the line
            // without its trailing whitespace.
            let trimmed = trim_end(trimmed);
            // The text after a block comment that closes on this line, if one does.
            let after_close = if let Some(close) = in_block {
                after(trimmed, close)
            } else if let Some((rest, close)) = style.block.iter().find_map(|&(open, close)| {
                Some((trimmed.strip_prefix(open.as_bytes())?, close.as_bytes()))
            }) {
                in_block = Some(close);
                after(rest, close)
            } else if style.line.iter().any(|p| trimmed.starts_with(p.as_bytes())) {
                counts.comment += 1;
                continue;
            } else {
                counts.code += 1;
                if !opens_after_code.is_empty() && may_open(line) {
                    in_block = self.open_after_code(trimmed);
                }
                continue;
            };
            match after_close {
                None => counts.comment += 1,
                Some(after) => {
                    if trim_start(after).is_empty() {
                        counts.comment += 1;
                        in_block = None;
                    } else {
                        counts.code += 1;
                        in_block = None;
                        if !opens_after_code.is_empty() && may_open(line) {
                            in_block = self.open_after_code(after);
                        }
                    }
                }
            }
        }
        counts
    }

    /// The sorted offsets of the lines of `content` that may end inside a block comment that
    /// opens after code on them.
    ///
    /// Such a line has an opening delimiter with something before it and no close after it. Some
    /// lines found are only inside strings or comments, which costs time but not accuracy.
    fn opens_after_code(&self, content: &[u8]) -> Vec<usize> {
        let mut offsets = Vec::new();
        for &(open, close) in self.style.block {
            let (open, close) = (open.as_bytes(), close.as_bytes());
            for at in memchr::memmem::find_iter(content, open) {
                // Most delimiters start their lines, which a look back over indentation finds.
                let before = content[..at].iter().rev().find(|&&b| b != b' ' && b != b'\t');
                if matches!(before, None | Some(b'\n')) {
                    continue;
                }
                let start = memchr::memrchr(b'\n', &content[..at]).map_or(0, |n| n + 1);
                let rest = &content[at + open.len()..];
                let end = memchr::memchr(b'\n', rest).unwrap_or(rest.len());
                if !trim_start(&content[start..at]).is_empty()
                    && find(&rest[..end], close).is_none()
                {
                    offsets.push(start);
                }
            }
        }
        offsets.sort_unstable();
        offsets.dedup();
        offsets
    }

    /// The close delimiter of a block comment that opens in `code` and is still open at its end.
    ///
    /// A delimiter after a line comment, or inside a string or character literal, does not open a
    /// comment. Strings end at the end of the line.
    fn open_after_code(&self, code: &[u8]) -> Option<&'static [u8]> {
        // With one kind of block comment, a line whose last opening delimiter is closed after it
        // ends outside every comment, whatever the quotes before it, and that is most lines with
        // one. Only the others are scanned.
        if let [(open, close)] = self.style.block
            && let Some(&first) = open.as_bytes().first()
        {
            let (open, close) = (open.as_bytes(), close.as_bytes());
            let last = memchr::memrchr_iter(first, code).find(|&i| code[i..].starts_with(open));
            match last {
                None => return None,
                Some(i) if after(&code[i + open.len()..], close).is_some() => return None,
                Some(_) => {}
            }
        }
        let mut i = 0;
        while let Some(&byte) = code.get(i) {
            if byte == b'"' {
                i += 1;
                while let Some(&byte) = code.get(i) {
                    i += if byte == b'\\' { 2 } else { 1 };
                    if byte == b'"' {
                        break;
                    }
                }
                continue;
            }
            // A character literal such as '"' or '\'' is skipped whole, and any other quote, such
            // as the start of a Rust lifetime, by itself.
            if byte == b'\'' {
                match (code.get(i + 1), code.get(i + 2), code.get(i + 3)) {
                    (Some(b'\\'), _, Some(b'\'')) => i += 4,
                    (Some(_), Some(b'\''), _) => i += 3,
                    _ => i += 1,
                }
                continue;
            }
            if self.starts[usize::from(byte)] {
                let rest = &code[i..];
                let block =
                    self.style.block.iter().find(|(open, _)| rest.starts_with(open.as_bytes()));
                if let Some((open, close)) = block {
                    match after(&rest[open.len()..], close.as_bytes()) {
                        Some(after) => {
                            i = code.len() - after.len();
                            continue;
                        }
                        None => return Some(close.as_bytes()),
                    }
                }
                if self.style.line.iter().any(|p| rest.starts_with(p.as_bytes())) {
                    return None;
                }
            }
            i += 1;
        }
        None
    }
}

/// The text after the first `close` in `text`, if there is one.
fn after<'a>(text: &'a [u8], close: &[u8]) -> Option<&'a [u8]> {
    find(text, close).map(|at| &text[at + close.len()..])
}

/// The offset of the first `delimiter` in `text`.
fn find(text: &[u8], delimiter: &[u8]) -> Option<usize> {
    // Delimiters are a few bytes, so finding the first byte and comparing the rest is faster than
    // building a substring searcher for each line or file.
    let Some((&first, rest)) = delimiter.split_first() else { return Some(0) };
    let mut from = 0;
    while let Some(i) = memchr::memchr(first, &text[from..]) {
        let at = from + i + 1;
        if text[at..].starts_with(rest) {
            return Some(at - 1);
        }
        from = at;
    }
    None
}

/// The lines of `content`, split as [`str::lines`] splits them, except that a line keeps a
/// trailing `\r`, which [`trim`] removes.
pub fn lines(content: &[u8]) -> impl Iterator<Item = &[u8]> {
    let mut start = 0;
    let ends = memchr::memchr_iter(b'\n', content).chain(std::iter::once(content.len()));
    ends.filter_map(move |end| {
        let line = content.get(start..end)?;
        start = end + 1;
        // The text after a final newline is not a line.
        (end < content.len() || !line.is_empty()).then_some(line)
    })
}

/// `text` without leading and trailing whitespace, as [`str::trim`] removes it.
///
/// Whitespace is checked byte by byte, and only an edge byte outside ASCII is decoded, so text
/// that is not UTF-8 is trimmed of ASCII whitespace alone.
pub fn trim(text: &[u8]) -> &[u8] {
    trim_end(trim_start(text))
}

/// `text` without leading whitespace, as [`str::trim_start`] removes it.
pub fn trim_start(mut text: &[u8]) -> &[u8] {
    while let Some(&first) = text.first() {
        let len = match first {
            b' ' | b'\t' | b'\n' | b'\x0B' | b'\x0C' | b'\r' => 1,
            0x80.. => match text.get(..utf8_width(first)).and_then(whitespace_char) {
                Some(len) => len,
                None => break,
            },
            _ => break,
        };
        text = &text[len..];
    }
    text
}

/// `text` without trailing whitespace, as [`str::trim_end`] removes it.
pub fn trim_end(mut text: &[u8]) -> &[u8] {
    while let Some(&last) = text.last() {
        let len = match last {
            b' ' | b'\t' | b'\n' | b'\x0B' | b'\x0C' | b'\r' => 1,
            0x80.. => {
                // A character is at most four bytes, and every byte after its first is a
                // continuation byte.
                let start = (text.len().saturating_sub(4)..text.len())
                    .rev()
                    .find(|&i| text[i] & 0xC0 != 0x80)
                    .unwrap_or(text.len() - 1);
                match whitespace_char(&text[start..]) {
                    Some(len) => len,
                    None => break,
                }
            }
            _ => break,
        };
        text = &text[..text.len() - len];
    }
    text
}

/// The number of bytes in the UTF-8 character that starts with `first`, or 1 if `first` cannot
/// start one.
pub fn utf8_width(first: u8) -> usize {
    match first {
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => 1,
    }
}

/// The length of `bytes` if they are exactly one whitespace character.
fn whitespace_char(bytes: &[u8]) -> Option<usize> {
    let mut chars = std::str::from_utf8(bytes).ok()?.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_whitespace() => Some(bytes.len()),
        _ => None,
    }
}

/// Map file extension to human-readable language name.
pub fn language_name(ext: &str) -> &str {
    match ext {
        "rs" => "Rust",
        "go" => "Go",
        "py" => "Python",
        "rb" | "rake" | "gemspec" => "Ruby",
        "erb" => "ERB",
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
        "dts" | "dtsi" => "Device Tree",
        // GNU and Intel style assembly are one language, as in cloc, because a reader wants to
        // know how much assembly there is more than which assembler reads it.
        "s" | "asm" => "Assembly",
        "rst" => "reStructuredText",
        "m4" => "M4",
        "autoconf" => "Autoconf",
        "automake" => "Automake",
        "awk" => "AWK",
        "l" | "lex" => "Lex",
        "y" | "yy" => "Yacc",
        "ld" | "lds" => "Linker Script",
        "tex" | "sty" => "TeX",
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
        "Rakefile" | "rakefile" | "Gemfile" => "rb".to_string(),
        "configure.ac" | "configure.in" => "autoconf".to_string(),
        "Makefile.am" => "automake".to_string(),
        _ => path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase(),
    }
}

/// Whether files with the key `key` are split into source and tests. Documentation,
/// configuration and build files are not.
pub fn is_split(key: &str) -> bool {
    !matches!(
        key,
        "md" | "markdown"
            | "rst"
            | "tex"
            | "sty"
            | "yaml"
            | "yml"
            | "toml"
            | "make"
            | "dockerfile"
            | "m4"
            | "autoconf"
            | "automake"
            | "ld"
            | "lds"
    )
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
        assert!(!is_counted("svg") && !is_counted("po"));
    }

    #[test]
    fn documentation_and_build_configuration_are_not_split() {
        for key in ["rst", "tex", "sty", "m4", "autoconf", "automake", "ld", "lds"] {
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
}
