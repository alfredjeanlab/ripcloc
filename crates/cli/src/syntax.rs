// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! Line counting with per-language comment detection.
//!
//! Comments are found line by line with simple heuristics rather than parsing, as `cloc` does.

use std::cell::Cell;

mod languages;
mod literals;

pub use languages::{
    LANGUAGES, file_key, is_counted, is_make_dependencies, is_split, language_name, pl_key,
    shebang_key, v_key,
};
use literals::{Literal, QUOTED, Special::*, literals};

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
        "rb" | "rake" | "gemspec" => {
            Some(CommentStyle { line: &["#"], block: &[("=begin", "=end")] })
        }
        "py" | "sh" | "bash" | "zsh" | "fish" | "bats" | "r" | "bzl" | "bazel" | "bzlmod"
        | "star" => Some(CommentStyle { line: &["#"], block: &[] }),
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "mts" | "cjs" | "cts" | "java" | "kt" | "scala"
        | "swift" | "c" | "cpp" | "h" | "hpp" | "cs" | "m" | "mm" | "dart" | "sol" | "gradle"
        | "v" | "vh" | "sv" | "svh" => Some(CommentStyle { line: &["//"], block: &[("/*", "*/")] }),
        "lua" => Some(CommentStyle { line: &["--"], block: &[("--[[", "]]")] }),
        "sql" => Some(CommentStyle { line: &["--"], block: &[("/*", "*/")] }),
        "php" => Some(CommentStyle { line: &["//", "#"], block: &[("/*", "*/")] }),
        "pl" | "pm" => Some(CommentStyle { line: &["#"], block: &[("=pod", "=cut")] }),
        "ps1" => Some(CommentStyle { line: &["#"], block: &[("<#", "#>")] }),
        "bat" | "cmd" => Some(CommentStyle { line: &["REM ", ":: "], block: &[] }),
        "toml" | "yaml" | "yml" | "make" | "dockerfile" | "cmake" => {
            Some(CommentStyle { line: &["#"], block: &[] })
        }
        "json" => Some(CommentStyle { line: &[], block: &[] }),
        // The `<script>` and `<style>` elements of HTML, Vue and Svelte are counted in their own
        // languages, so this style is for the markup around them.
        // MSBuild projects and their imports, XAML, resources and NuGet specifications are XML.
        "md" | "markdown" | "html" | "htm" | "xml" | "vue" | "svelte" | "csproj" | "vbproj"
        | "fsproj" | "props" | "targets" | "xaml" | "resx" | "nuspec" => {
            Some(CommentStyle { line: &[], block: &[("<!--", "-->")] })
        }
        "css" => Some(CommentStyle { line: &[], block: &[("/*", "*/")] }),
        "scss" | "sass" | "less" | "proto" | "cc" | "cxx" | "hh" | "kts" => {
            Some(CommentStyle { line: &["//"], block: &[("/*", "*/")] })
        }
        "ini" => Some(CommentStyle { line: &[";", "#"], block: &[] }),
        "awk" | "automake" => Some(CommentStyle { line: &["#"], block: &[] }),
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
        "zig" | "zon" => Some(CommentStyle { line: &["//"], block: &[] }),
        "hs" => Some(CommentStyle { line: &["--"], block: &[("{-", "-}")] }),
        "lean" => Some(CommentStyle { line: &["--"], block: &[("/-", "-/")] }),
        "ml" | "mli" | "coq" => Some(CommentStyle { line: &[], block: &[("(*", "*)")] }),
        "ex" | "exs" | "tfvars" => Some(CommentStyle { line: &["#"], block: &[] }),
        "jl" => Some(CommentStyle { line: &["#"], block: &[("#=", "=#")] }),
        "nix" | "pp" => Some(CommentStyle { line: &["#"], block: &[("/*", "*/")] }),
        "tf" => Some(CommentStyle { line: &["#", "//"], block: &[("/*", "*/")] }),
        "clj" | "cljs" | "cljc" => Some(CommentStyle { line: &[";"], block: &[] }),
        "erl" | "hrl" => Some(CommentStyle { line: &["%"], block: &[] }),
        // MDX is Markdown with JSX, whose comments are written `{/* ... */}`.
        "mdx" => Some(CommentStyle { line: &[], block: &[("<!--", "-->"), ("{/*", "*/}")] }),
        // Keywords in Visual Basic ignore case, so `REM` is matched in its common spellings.
        "vb" => Some(CommentStyle { line: &["'", "REM ", "Rem ", "rem "], block: &[] }),
        "adoc" | "asciidoc" => Some(CommentStyle { line: &["//"], block: &[("////", "////")] }),
        "j2" | "jinja" | "jinja2" => Some(CommentStyle { line: &[], block: &[("{#", "#}")] }),
        // The arms from here on are of languages that are rarer than those above, so that a lookup
        // for a common language finds its arm before them.
        "hxx" | "h++" | "c++" | "inl" | "ipp" | "tpp" | "cppm" | "ccm" | "mxx" | "ixx" | "csx"
        | "sc" | "cu" | "cuh" | "groovy" | "gvy" | "gy" | "gsh" | "glsl" | "vert" | "frag"
        | "geom" | "comp" | "tesc" | "tese" | "hlsl" | "fx" | "fxh" | "metal" | "typ" => {
            Some(CommentStyle { line: &["//"], block: &[("/*", "*/")] })
        }
        "pyi" | "pyw" | "pyx" | "pxd" | "pxi" | "scons" | "just" | "sln" | "tcl" | "tk"
        | "graphql" | "gql" | "cr" => Some(CommentStyle { line: &["#"], block: &[] }),
        "gleam" => Some(CommentStyle { line: &["//"], block: &[] }),
        "psm1" | "psd1" => Some(CommentStyle { line: &["#"], block: &[("<#", "#>")] }),
        "luau" => Some(CommentStyle { line: &["--"], block: &[("--[[", "]]")] }),
        "vcxproj" | "ps1xml" => Some(CommentStyle { line: &[], block: &[("<!--", "-->")] }),
        "hcl" => Some(CommentStyle { line: &["#", "//"], block: &[("/*", "*/")] }),
        "fs" | "fsi" | "fsx" => Some(CommentStyle { line: &["//"], block: &[("(*", "*)")] }),
        // Fixed-form Fortran and COBOL also mark a comment in a column of their own, which
        // `count_fixed_form` reads.
        "f90" | "f95" | "f03" | "f08" | "f" | "for" | "f77" => {
            Some(CommentStyle { line: &["!"], block: &[] })
        }
        "cbl" | "cob" | "cobol" | "cpy" => Some(CommentStyle { line: &["*>"], block: &[] }),
        "pas" | "dpr" | "lpr" => {
            Some(CommentStyle { line: &["//"], block: &[("{", "}"), ("(*", "*)")] })
        }
        // D's `/+` comments nest, and its `/*` comments do not, but both are taken to nest, since
        // a flag for each delimiter would cost every language a lookup for each comment.
        "d" | "di" => Some(CommentStyle { line: &["//"], block: &[("/*", "*/"), ("/+", "+/")] }),
        "el" => Some(CommentStyle { line: &[";"], block: &[] }),
        "scm" | "ss" | "sld" | "sls" | "rkt" | "lisp" | "lsp" | "asd" => {
            Some(CommentStyle { line: &[";"], block: &[("#|", "|#")] })
        }
        "adb" | "ads" | "ada" => Some(CommentStyle { line: &["--"], block: &[] }),
        // A `{{!` comment ends at the first `}}`, and a `{{!--` comment ends only at `--}}`.
        "hbs" | "handlebars" => Some(CommentStyle {
            line: &[],
            block: &[("{{!--", "--}}"), ("{{!", "}}"), ("<!--", "-->")],
        }),
        "mustache" => Some(CommentStyle { line: &[], block: &[("{{!", "}}"), ("<!--", "-->")] }),
        // Pug's `//-` comments, which are left out of the HTML, start with `//` as its other
        // comments do.
        "pug" | "jade" => Some(CommentStyle { line: &["//"], block: &[] }),
        "coffee" => Some(CommentStyle { line: &["#"], block: &[("###", "###")] }),
        "elm" | "purs" => Some(CommentStyle { line: &["--"], block: &[("{-", "-}")] }),
        // VHDL-2008 added C's block comments.
        "vhd" | "vhdl" => Some(CommentStyle { line: &["--"], block: &[("/*", "*/")] }),
        "thrift" => Some(CommentStyle { line: &["//", "#"], block: &[("/*", "*/")] }),
        "nim" | "nims" | "nimble" => {
            Some(CommentStyle { line: &["#"], block: &[("##[", "]##"), ("#[", "]#")] })
        }
        "odin" => Some(CommentStyle { line: &["//"], block: &[("/*", "*/")] }),
        // Unlike strict JSON, JSONC and JSON5 have C's comments.
        "jsonc" | "json5" | "styl" | "qml" | "hx" => {
            Some(CommentStyle { line: &["//"], block: &[("/*", "*/")] })
        }
        "mk" | "mak" | "ksh" | "csh" | "tcsh" | "meson.build" | "nu" => {
            Some(CommentStyle { line: &["#"], block: &[] })
        }
        // Prisma's `///` comments document a model, and start with `//` as its other comments do.
        "prisma" => Some(CommentStyle { line: &["//"], block: &[] }),
        // A line that starts with `"` is a comment in legacy Vim script, and Vim9 script writes its
        // comments with `#`. No string or comment of either spans lines, so a `"` after code, which
        // starts a string or a comment, does not change how a later line is counted.
        "vim" => Some(CommentStyle { line: &["\"", "#"], block: &[] }),
        "jsonnet" | "libsonnet" => {
            Some(CommentStyle { line: &["//", "#"], block: &[("/*", "*/")] })
        }
        // Forth's `\` and `(` are words, which end at a space. A line that starts with `\` is
        // taken for a comment, which also covers gforth's `\G` documentation. A `(` comment is
        // recognized only at the start of a line, since a stack comment such as `( n -- n )` after
        // the name of a definition closes on its line.
        "fth" | "4th" | "forth" => Some(CommentStyle { line: &["\\"], block: &[("( ", ")")] }),
        // Smalltalk is counted by `count_smalltalk`, which knows that `$"` is a character.
        "st" => Some(CommentStyle { line: &[], block: &[("\"", "\"")] }),
        "prolog" => Some(CommentStyle { line: &["%"], block: &[("/*", "*/")] }),
        // A notebook is counted by `notebook::count`, which counts each cell in its own language,
        // and not with this style.
        "ipynb" => Some(CommentStyle { line: &[], block: &[] }),
        // The code between `<%` and `%>` in ASP.NET Web Forms is counted with its markup, as in
        // ERB.
        "aspx" | "ascx" | "asmx" | "ashx" | "asax" | "master" => {
            Some(CommentStyle { line: &[], block: &[("<%--", "--%>"), ("<!--", "-->")] })
        }
        // Razor's C# comments are recognized in its markup too, since telling the C# of `@{ }`
        // and `@code { }` blocks from the markup around them would need its braces matched.
        "cshtml" | "vbhtml" | "razor" => Some(CommentStyle {
            line: &["//"],
            block: &[("@*", "*@"), ("<!--", "-->"), ("/*", "*/")],
        }),
        // A line of classic ASP that starts with `'` is taken for a VBScript comment, whether or
        // not it is inside `<% %>`, since markup rarely starts a line with a quote.
        "asp" => Some(CommentStyle { line: &["'"], block: &[("<!--", "-->")] }),
        "svg" => Some(CommentStyle { line: &[], block: &[("<!--", "-->")] }),
        "as" => Some(CommentStyle { line: &["//"], block: &[("/*", "*/")] }),
        // ColdFusion's own comments have three dashes, and are left out of the HTML that it
        // writes, while the HTML comments around them have two.
        "cfm" | "cfml" => {
            Some(CommentStyle { line: &[], block: &[("<!---", "--->"), ("<!--", "-->")] })
        }
        // A component is written in CFScript or in tags, and the comments of both are recognized,
        // since neither starts a line with the other's comment delimiters.
        "cfc" => Some(CommentStyle { line: &["//"], block: &[("/*", "*/"), ("<!---", "--->")] }),
        "twig" => Some(CommentStyle { line: &[], block: &[("{#", "#}")] }),
        _ => None,
    }
}

/// Count the lines of `content`, using the comment syntax for the extension `ext`.
///
/// A line with code is code, even when a block comment opens or closes on it, and the lines
/// after it inside the comment are comments. Every non-blank line inside a string literal is code,
/// even when it looks like a comment, except in a Python docstring, which is a string that is a
/// statement of its own and whose lines are comments. Without a known comment syntax, every
/// non-blank line is code. The content need not be UTF-8, and a leading UTF-8 byte order mark is
/// ignored.
///
/// In HTML, Vue and Svelte, the lines inside a `<script>` or `<style>` element are counted in the
/// language of the element, and the lines that open and close it are markup.
pub fn count_lines(content: &(impl AsRef<[u8]> + ?Sized), ext: &str) -> LineCounts {
    let content = content.as_ref();
    let content = content.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(content);
    match ext {
        "html" | "htm" | "vue" | "svelte" => count_markup(content, ext),
        "f" | "for" | "f77" => count_fixed_form(content, 0, b"Cc*!", b"!"),
        "cbl" | "cob" | "cobol" | "cpy" => count_fixed_form(content, 6, b"*/", b"*>"),
        "st" => count_smalltalk(content),
        _ => count_in(content, ext, None),
    }
}

/// Count the lines of `content`, Smalltalk, whose comments are written `"..."` and whose strings
/// are written `'...'`. A quote that is doubled inside either stands for itself. A `$` makes the
/// character after it a character literal, so `$"` and `$'` open neither, which the comment syntax
/// of [`count_in`] cannot tell.
///
/// A line is a comment when every byte on it that is not whitespace is inside a comment.
fn count_smalltalk(content: &[u8]) -> LineCounts {
    let mut counts = LineCounts::default();
    // The quote of the comment or string that the scanner is inside.
    let mut inside: Option<u8> = None;
    for line in lines(content) {
        if trim(line).is_empty() {
            counts.blank += 1;
            continue;
        }
        // A doubled quote closes and opens again at once, so it needs no rule of its own.
        let mut code = inside == Some(b'\'');
        let mut i = 0;
        while let Some(&byte) = line.get(i) {
            match (inside, byte) {
                (Some(quote), _) if byte == quote => inside = None,
                (Some(_), _) => {}
                (None, b'"' | b'\'') => {
                    inside = Some(byte);
                    code |= byte == b'\'';
                }
                (None, b'$') => {
                    code = true;
                    i += 1;
                }
                (None, _) => code |= !byte.is_ascii_whitespace(),
            }
            i += 1;
        }
        if code {
            counts.code += 1;
        } else {
            counts.comment += 1;
        }
    }
    counts
}

/// Count the lines of `content`, fixed-form source in which a line whose byte at `column` is one
/// of `marks` is a comment, and so is a line that starts with `line_comment` after its
/// indentation.
///
/// Fixed-form Fortran marks a comment with `C`, `c`, `*` or `!` in the first column, and COBOL
/// with `*` or `/` in its indicator column, the seventh, after a sequence number. Neither has
/// block comments or strings that span lines, so a line is told apart by its first bytes alone.
fn count_fixed_form(
    content: &[u8],
    column: usize,
    marks: &[u8],
    line_comment: &[u8],
) -> LineCounts {
    let mut counts = LineCounts::default();
    for line in lines(content) {
        let trimmed = trim_start(line);
        if trimmed.is_empty() {
            counts.blank += 1;
        } else if line.get(column).is_some_and(|b| marks.contains(b))
            || trimmed.starts_with(line_comment)
        {
            counts.comment += 1;
        } else {
            counts.code += 1;
        }
    }
    counts
}

/// Count the lines of `content`, which has no byte order mark, in the language of `ext` alone.
///
/// Onto `code_starts` go the offsets of lines that start in code outside every comment and string,
/// in order, each with the counts of the lines before it. Not every such line is pushed, but every
/// one that starts with `#` is.
pub fn count_in(content: &[u8], ext: &str, code_starts: Option<&mut CodeStarts>) -> LineCounts {
    let syntax = Syntax::new(ext);
    let opens_after_code =
        if syntax.c_like { syntax.opens_after_code(content) } else { Vec::new() };
    // The loop that does not push the lines is a copy of its own, which pays nothing for them, and
    // so is the loop of a language without docstrings.
    let offsets = &opens_after_code;
    match (code_starts, syntax.docstrings) {
        (Some(code_starts), false) => syntax.count::<true, false>(content, offsets, code_starts),
        (Some(code_starts), true) => syntax.count::<true, true>(content, offsets, code_starts),
        (None, false) => syntax.count::<false, false>(content, offsets, &mut Vec::new()),
        (None, true) => syntax.count::<false, true>(content, offsets, &mut Vec::new()),
    }
}

/// The lines that [`count_in`] pushes onto its `code_starts`.
type CodeStarts = Vec<(usize, LineCounts)>;

/// Count the lines of `content`, markup in the language of `ext` that has `<script>` and `<style>`
/// elements, whose bodies are counted in their own languages.
///
/// An element has a body only when its opening tag ends on the line where it starts and the
/// element does not close on that line. The body ends at the first line with the closing tag, as it
/// does in a browser, even when the tag is inside a string or comment of the embedded language.
fn count_markup(content: &[u8], ext: &str) -> LineCounts {
    let mut counts = LineCounts::default();
    let offset = |line: &[u8]| line.as_ptr().addr() - content.as_ptr().addr();
    // The start of the markup that has not been counted.
    let mut markup = 0;
    let mut in_comment = false;
    // The closing tag, language and start of the body of the element that the line is inside.
    let mut element: Option<(&[u8], &str, usize)> = None;
    for line in lines(content) {
        let at = offset(line);
        // The markup on the line, which follows the closing tag of an element that ends on it.
        let mut tags = line;
        if let Some((close, language, body)) = element {
            let Some(end) = find_ignore_case(line, close) else { continue };
            counts.add(count_in(content.get(body..at).unwrap_or_default(), language, None));
            (markup, element) = (at, None);
            tags = line.get(end + close.len()..).unwrap_or_default();
        }
        if let Some((close, language)) = element_opening(tags, &mut in_comment) {
            let body = (at + line.len() + 1).min(content.len());
            counts.add(count_in(content.get(markup..body).unwrap_or_default(), ext, None));
            element = Some((close, language, body));
        }
    }
    let (rest, language) = match element {
        Some((_, language, body)) => (body, language),
        None => (markup, ext),
    };
    counts.add(count_in(content.get(rest..).unwrap_or_default(), language, None));
    counts
}

/// The closing tag and language of the `<script>` or `<style>` element whose body starts after
/// `line`, or `None` if no body starts there. `in_comment` says whether the line starts inside an
/// HTML comment, and is updated to say whether the next line does.
fn element_opening(line: &[u8], in_comment: &mut bool) -> Option<(&'static [u8], &'static str)> {
    const ELEMENTS: [(&[u8], &[u8]); 2] = [(b"<script", b"</script"), (b"<style", b"</style")];
    let mut i = 0;
    loop {
        if *in_comment {
            i += find(line.get(i..)?, b"-->")? + 3;
            *in_comment = false;
        }
        i += memchr::memchr(b'<', line.get(i..)?)?;
        let tail = line.get(i..)?;
        if tail.starts_with(b"<!--") {
            (i, *in_comment) = (i + 4, true);
            continue;
        }
        let tag = ELEMENTS.iter().find(|(open, _)| {
            starts_with_ignore_case(tail, open)
                && tail.get(open.len()).is_none_or(|&b| b == b'>' || b == b'/' || b <= b' ')
        });
        let Some(&(open, close)) = tag else {
            i += 1;
            continue;
        };
        // A tag that does not end on its line is taken for markup.
        let gt = memchr::memchr(b'>', tail)?;
        let attributes = tail.get(open.len()..gt)?;
        i += gt + 1;
        if attributes.ends_with(b"/") {
            continue;
        }
        match find_ignore_case(line.get(i..)?, close) {
            Some(end) => i += end + close.len(),
            None => return Some((close, element_language(open == b"<script", attributes))),
        }
    }
}

/// The extension of the language of a `<script>` element, if `script`, or else of a `<style>`
/// element, with the `attributes` in its opening tag. The extension of an unknown language is
/// empty, so that every line of the element is code.
fn element_language(script: bool, attributes: &[u8]) -> &'static str {
    // The values of `lang` that name a language, with the extension of each.
    const LANGUAGES: &[(&str, &str)] = &[
        ("ts", "ts"),
        ("typescript", "ts"),
        ("tsx", "tsx"),
        ("js", "js"),
        ("javascript", "js"),
        ("jsx", "jsx"),
        ("css", "css"),
        ("postcss", "css"),
        ("scss", "scss"),
        ("sass", "sass"),
        ("less", "less"),
    ];
    if let Some(lang) = attribute(attributes, b"lang") {
        let known = LANGUAGES.iter().find(|(name, _)| lang.eq_ignore_ascii_case(name.as_bytes()));
        return known.map_or("", |&(_, ext)| ext);
    }
    if !script {
        return "css";
    }
    let Some(kind) = attribute(attributes, b"type") else { return "js" };
    let kind = kind.to_ascii_lowercase();
    let contains = |part: &[u8]| memchr::memmem::find(&kind, part).is_some();
    if kind.is_empty() || kind == b"module" || contains(b"javascript") || contains(b"ecmascript") {
        "js"
    } else if contains(b"typescript") {
        "ts"
    } else if contains(b"babel") || contains(b"jsx") {
        "jsx"
    } else if contains(b"json") {
        "json"
    } else if contains(b"html") || contains(b"template") {
        "html"
    } else {
        ""
    }
}

/// The value of the attribute `name` in the `attributes` of a tag, without its quotes.
fn attribute<'a>(attributes: &'a [u8], name: &[u8]) -> Option<&'a [u8]> {
    let mut i = 0;
    loop {
        i += find_ignore_case(attributes.get(i..)?, name)?;
        let after = attributes.get(i + name.len()..)?;
        let spaced = i.checked_sub(1).and_then(|p| attributes.get(p)).is_some_and(|&b| b <= b' ');
        i += name.len();
        let Some(value) = after.trim_ascii_start().strip_prefix(b"=").filter(|_| spaced) else {
            continue;
        };
        let value = value.trim_ascii_start();
        return Some(match value.first() {
            Some(&quote @ (b'"' | b'\'')) => {
                let value = value.get(1..)?;
                value.get(..memchr::memchr(quote, value).unwrap_or(value.len()))?
            }
            _ => value.get(..value.iter().position(|&b| b <= b' ').unwrap_or(value.len()))?,
        });
    }
}

/// Whether `text` starts with `prefix`, ignoring ASCII case.
fn starts_with_ignore_case(text: &[u8], prefix: &[u8]) -> bool {
    text.get(..prefix.len()).is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

/// The offset of the first `needle` in `text`, ignoring ASCII case.
fn find_ignore_case(text: &[u8], needle: &[u8]) -> Option<usize> {
    let first = *needle.first()?;
    let (lower, upper) = (first.to_ascii_lowercase(), first.to_ascii_uppercase());
    memchr::memchr2_iter(lower, upper, text)
        .find(|&at| starts_with_ignore_case(text.get(at..).unwrap_or_default(), needle))
}

/// The lines of a text in one language, each with its code, which is the line without its comments
/// and without its string and character literals.
///
/// The code of a line is found only when [`CodeLines::code`] asks for it, because a line whose
/// code is not needed is only scanned for the state after it, which costs less.
pub struct CodeLines<'a> {
    lines: Lines<'a>,
    syntax: Syntax,
    state: State,
    /// The last line returned, if it has not been scanned yet.
    unscanned: Option<&'a [u8]>,
    /// The code of the last line when it is a single run of the line, which needs no copy.
    run: &'a [u8],
    /// The code of the last line when it joins more than one run, and is empty otherwise.
    joined: Vec<u8>,
}

impl<'a> CodeLines<'a> {
    /// The lines of `content` in the language of `ext`.
    pub fn new(content: &'a [u8], ext: &str) -> Self {
        let (syntax, state) = (Syntax::new(ext), State::default());
        Self { lines: lines(content), syntax, state, unscanned: None, run: &[], joined: Vec::new() }
    }

    /// Go on with the lines of `rest`, a later part of the text that starts in code outside every
    /// comment and string.
    pub fn restart(&mut self, rest: &'a [u8]) {
        (self.lines, self.state, self.unscanned) = (lines(rest), State::default(), None);
    }

    /// The next line, split as [`lines`] splits them, or `None` after the last.
    pub fn next_line(&mut self) -> Option<&'a [u8]> {
        if let Some(line) = self.unscanned.take()
            && self.syntax.may_leave(&self.state, line)
        {
            self.state = self.syntax.scan(line, self.state, &mut |_| {});
        }
        self.unscanned = self.lines.next();
        self.unscanned
    }

    /// The code of the line that [`CodeLines::next_line`] returned last.
    pub fn code(&mut self) -> &[u8] {
        if let Some(line) = self.unscanned.take() {
            let (run, joined) = (&mut self.run, &mut self.joined);
            *run = &[];
            joined.clear();
            self.state = self.syntax.scan(line, self.state, &mut |code| {
                if run.is_empty() {
                    *run = code;
                } else if !code.is_empty() {
                    if joined.is_empty() {
                        joined.extend_from_slice(run);
                    }
                    joined.extend_from_slice(code);
                }
            });
        }
        if self.joined.is_empty() { self.run } else { &self.joined }
    }
}

/// A string that the scanner is inside, with the `escape` and `spans` of its [`Literal`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Str {
    /// The first `len` bytes close the string.
    close: [u8; 18],
    len: u8,
    escape: u8,
    spans: bool,
}

impl Str {
    /// A string that closes at the concatenation of `parts`, or `None` if that does not fit.
    fn new(parts: &[&[u8]], escape: u8, spans: bool) -> Option<Self> {
        let (mut close, mut len) = ([0; 18], 0);
        for part in parts {
            close.get_mut(len..len + part.len())?.copy_from_slice(part);
            len += part.len();
        }
        Some(Str { close, len: u8::try_from(len).ok()?, escape, spans })
    }

    /// The offset in `text` after the end of the string, whose contents continue at `from`, or
    /// `None` if it does not end in `text`.
    fn end(&self, text: &[u8], from: usize) -> Option<usize> {
        let close = self.close.get(..usize::from(self.len))?;
        let (&first, escape) = (close.first()?, self.escape);
        let mut i = from;
        loop {
            let rest = text.get(i..)?;
            let at = i + match escape {
                0 => memchr::memchr(first, rest)?,
                _ => memchr::memchr2(first, escape, rest)?,
            };
            let tail = text.get(at..)?;
            // A doubled escape stands for one byte, and so does a doubled quote that is its own
            // escape.
            i = match tail {
                [quote, again, ..] if *quote == escape && again == quote => at + 2,
                _ if starts_with(tail, close) => return Some(at + close.len()),
                [byte, ..] if *byte == escape => at + 2,
                _ => at + 1,
            };
        }
    }
}

/// The offset in `text` after the end of the template literal whose contents continue at `from`,
/// or after the `${` that opens a substitution in it, or `None` if neither is in `text`.
fn template_end(text: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    loop {
        let at = i + memchr::memchr3(b'`', b'\\', b'$', text.get(i..)?)?;
        match text.get(at..) {
            Some([b'`', ..]) => return Some(at + 1),
            Some([b'$', b'{', ..]) => return Some(at + 2),
            Some([b'\\', ..]) => i = at + 2,
            _ => i = at + 1,
        }
    }
}

/// The mode inside the C++ raw string that the `"` at `quote` in `code` opens, and the offset of
/// its contents, or `None` if the quote does not open one.
fn cpp_raw_string_at(code: &[u8], quote: usize) -> Option<(Mode, usize)> {
    let r = quote.checked_sub(1).filter(|&r| code.get(r) == Some(&b'R'))?;
    let start = match code.get(..r) {
        Some(head) if head.ends_with(b"u8") => r - 2,
        Some([.., b'u' | b'U' | b'L']) => r - 1,
        _ => r,
    };
    if ident_before(code, start) {
        return None;
    }
    let rest = code.get(quote + 1..)?;
    // A delimiter is at most 16 bytes and has no space, parenthesis or backslash.
    let len = rest.iter().take(17).position(|&b| b == b'(')?;
    let name = rest.get(..len)?;
    if name.iter().any(|b| b" )\\\t\x0B\x0C\"".contains(b)) {
        return None;
    }
    Some((Mode::Str(Str::new(&[b")", name, b"\""], 0, true)?), quote + 2 + len))
}

/// Whether the byte before `at` in `code` can be part of an identifier.
fn ident_before(code: &[u8], at: usize) -> bool {
    at.checked_sub(1)
        .and_then(|p| code.get(p))
        .is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'_')
}

/// The offset after the character literal whose `'` is at `quote` in `code`, such as `'"'`,
/// `'\''`, `'\u{7b}'` or `'é'`, or `None` if none starts there, as when the quote starts a Rust
/// lifetime.
fn char_literal_end(code: &[u8], quote: usize) -> Option<usize> {
    let rest = code.get(quote + 1..)?;
    let len = match *rest.first()? {
        // The longest escape is `\u{10FFFF}`, and the quote in `\'` does not close the literal.
        b'\\' => 2 + rest.iter().skip(2).take(9).position(|&b| b == b'\'')?,
        first => utf8_width(first),
    };
    (rest.get(len) == Some(&b'\'')).then_some(quote + 2 + len)
}

/// A block comment that the scanner is inside, `depth` levels deep in comments that it nests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Comment {
    open: &'static [u8],
    close: &'static [u8],
    depth: u32,
}

/// Where the scanner is between lines and within a line.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Mode {
    #[default]
    Code,
    Block(Comment),
    Str(Str),
    /// Inside the text of a JavaScript template literal.
    Template,
}

/// The scanner's [`Mode`], and the template literal substitutions that it is inside.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct State {
    mode: Mode,
    /// A stack of bytes, innermost lowest, each one more than the number of `{` that are open in
    /// its substitution. A substitution nested deeper than the stack holds is taken for text.
    substitutions: u64,
}

/// The comment and string syntax of a language, with the lookups that counting needs.
struct Syntax {
    style: CommentStyle,
    literals: &'static [Literal],
    /// Whether block comments nest, as in Rust, Swift, Haskell and OCaml.
    nests: bool,
    /// Whether the language is JavaScript, whose regular expressions hide quotes.
    javascript: bool,
    /// The first bytes of every comment delimiter, so that most bytes and most lines of code are
    /// told apart by one lookup.
    starts: [bool; 256],
    /// The bytes in `starts`, and those that the scanner stops at for strings.
    special: [bool; 256],
    /// The bytes in `special`, in order. When there are at most three, they are found with
    /// `memchr`, which is faster than looking up every byte in `special`.
    special_bytes: Vec<u8>,
    /// The quotes that can open a string that spans lines, among which is the `|` of Jsonnet's
    /// text blocks.
    spanning: Vec<u8>,
    /// The bytes in `spanning`, and the first bytes of the block comment delimiters, without
    /// which a line that starts in code ends in code.
    openers: Vec<u8>,
    /// Whether a block comment can open after code, as C's `/*` and OCaml's `(*` do. An HTML
    /// comment cannot, since a Markdown code block may show one opening.
    c_like: bool,
    /// Whether a string that is a statement of its own is documentation, as a docstring is in
    /// Python and in the languages that share its syntax.
    docstrings: bool,
}

impl Syntax {
    fn new(ext: &str) -> Self {
        let style = comment_style(ext).unwrap_or(CommentStyle { line: &[], block: &[] });
        let c_like =
            style.block.iter().any(|&(open, _)| matches!(open, "/*" | "(*" | "{-" | "/-" | "#="));
        let literals = match literals(ext) {
            [] if c_like => &[QUOTED],
            literals => literals,
        };
        let javascript = literals.iter().any(|l| matches!(l.special, Template));
        let mut starts = [false; 256];
        let delimiters =
            style.block.iter().map(|&(open, _)| open).chain(style.line.iter().copied());
        let mut special_bytes: Vec<u8> =
            delimiters.filter_map(|delimiter| delimiter.as_bytes().first().copied()).collect();
        for &byte in &special_bytes {
            mark(&mut starts, byte);
        }
        let quotes = literals.iter().filter_map(|l| l.open.first().copied());
        let braces = if javascript { b"{}".as_slice() } else { b"" };
        let char_quote = if literals.is_empty() { b"".as_slice() } else { b"'" };
        special_bytes
            .extend(quotes.chain(braces.iter().copied()).chain(char_quote.iter().copied()));
        special_bytes.sort_unstable();
        special_bytes.dedup();
        let mut special = [false; 256];
        for &byte in &special_bytes {
            mark(&mut special, byte);
        }
        let spanning: Vec<u8> = b"\"'`|"
            .iter()
            .copied()
            .filter(|&q| literals.iter().any(|l| l.spans && l.open.first() == Some(&q)))
            .collect();
        let block_starts = style.block.iter().filter_map(|&(open, _)| open.as_bytes().first());
        let mut openers: Vec<u8> = spanning.iter().chain(block_starts).copied().collect();
        openers.sort_unstable();
        openers.dedup();
        let nests = matches!(
            ext,
            "rs" | "swift"
                | "kt"
                | "kts"
                | "scala"
                | "dart"
                | "hs"
                | "ml"
                | "mli"
                | "jl"
                | "lean"
                | "coq"
                | "fs"
                | "fsi"
                | "fsx"
                | "sc"
                | "scm"
                | "ss"
                | "sld"
                | "sls"
                | "rkt"
                | "lisp"
                | "lsp"
                | "asd"
                | "elm"
                | "purs"
                | "nim"
                | "nims"
                | "nimble"
                | "odin"
                | "d"
                | "di"
        );
        Self {
            style,
            literals,
            nests,
            javascript,
            starts,
            special,
            special_bytes,
            spanning,
            openers,
            c_like,
            docstrings: matches!(
                ext,
                "py" | "pyi"
                    | "pyw"
                    | "pyx"
                    | "pxd"
                    | "pxi"
                    | "bzl"
                    | "bazel"
                    | "bzlmod"
                    | "star"
                    | "scons"
            ),
        }
    }

    /// Whether a comment delimiter of the language starts with `byte`.
    fn starts_with(&self, byte: u8) -> bool {
        // The table has an entry for every byte, so the fallback is never taken.
        self.starts.get(usize::from(byte)).copied().unwrap_or(true)
    }

    /// Whether the scanner stops at `byte`.
    fn is_special(&self, byte: u8) -> bool {
        self.special.get(usize::from(byte)).copied().unwrap_or(true)
    }

    /// Whether [`Syntax::scan`] can end in another state than `state` after `text`. It cannot when
    /// `text` starts in code outside every substitution and has none of the `openers`.
    fn may_leave(&self, state: &State, text: &[u8]) -> bool {
        if state.mode != Mode::Code || state.substitutions != 0 {
            return true;
        }
        match *self.openers {
            [] => false,
            [a] => memchr::memchr(a, text).is_some(),
            [a, b] => memchr::memchr2(a, b, text).is_some(),
            [a, b, c] => memchr::memchr3(a, b, c, text).is_some(),
            _ => true,
        }
    }

    /// The offset of the first byte in `text` that the scanner stops at.
    fn find_special(&self, text: &[u8]) -> Option<usize> {
        match *self.special_bytes {
            [] => None,
            [a] => memchr::memchr(a, text),
            [a, b] => memchr::memchr2(a, b, text),
            [a, b, c] => memchr::memchr3(a, b, c, text),
            _ => text.iter().position(|&b| self.is_special(b)),
        }
    }

    /// Count the lines of `content`, where a block comment can open after code only on the lines
    /// that start at the sorted offsets `opens_after_code`, and where `DOCSTRINGS` is whether the
    /// language has docstrings.
    fn count<const PUSH: bool, const DOCSTRINGS: bool>(
        &self,
        content: &[u8],
        opens_after_code: &[usize],
        code_starts: &mut CodeStarts,
    ) -> LineCounts {
        let style = &self.style;
        let mut counts = LineCounts::default();
        let mut state = State::default();
        let mut offsets = opens_after_code.iter().copied().peekable();
        // The offset of the first byte at or after the cursor that can open a string that spans
        // lines. Finding it once for many lines keeps the cost of strings to the lines with one.
        let spanning = self.spanning.as_slice();
        let mut next_opener = find_any(content, 0, spanning);
        // The lesser of `next_opener` and the next offset in `opens_after_code`, so that a line
        // that ends before it is told apart from the lines to scan by one comparison.
        let next_scan = Cell::new(next_opener.min(offsets.peek().copied().unwrap_or(usize::MAX)));
        // Whether the rest of `line` from a point outside every comment and string, where `state`
        // holds any substitutions that the point is inside, must be scanned for the state after
        // it.
        let mut needs_scan = |line: &[u8], state: &State| {
            let start = line.as_ptr().addr() - content.as_ptr().addr();
            let end = start + line.len();
            if state.substitutions == 0 && end < next_scan.get() {
                return false;
            }
            if next_opener < start {
                next_opener = find_any(content, start, spanning);
            }
            while offsets.next_if(|&offset| offset < start).is_some() {}
            let next_open = offsets.peek().copied();
            next_scan.set(next_opener.min(next_open.unwrap_or(usize::MAX)));
            state.substitutions != 0 || next_opener < end || next_open == Some(start)
        };

        // The number of lines so far of the string that may be a docstring, which become comment
        // lines if its closing quotes end its statement.
        let mut docstring_lines: Option<usize> = None;
        // The offset of the last line that starts inside a string, with that string, and the
        // offset of the last line on which a docstring ends. Only languages with docstrings keep
        // them.
        let (mut last_str, mut last_docstring) = (None, usize::MAX);
        let mut lines = lines(content);
        while let Some(mut line) = lines.next() {
            // A line that ends at or after `next_scan` is told apart here, so that it does not cost
            // a call to `count_plain` that returns at once.
            let end = line.as_ptr().addr() - content.as_ptr().addr() + line.len();
            if end < next_scan.get() && state == State::default() {
                let until = next_scan.get();
                let next = self.count_plain::<PUSH>(
                    content,
                    line,
                    &mut lines,
                    until,
                    &mut counts,
                    code_starts,
                );
                let Some(next) = next else { break };
                line = next;
            }
            if PUSH && state == State::default() {
                code_starts.push((line.as_ptr().addr() - content.as_ptr().addr(), counts));
            }
            let trimmed = trim_start(line);
            let Some(&first) = trimmed.first() else {
                counts.blank += 1;
                continue;
            };
            match state.mode {
                Mode::Str(s) if DOCSTRINGS && let Some(lines) = docstring_lines => {
                    counts.code += 1;
                    let offset = line.as_ptr().addr() - content.as_ptr().addr();
                    last_str = Some((offset, s));
                    let Some(end) = s.end(trimmed, 0) else {
                        docstring_lines = Some(lines + 1);
                        continue;
                    };
                    docstring_lines = None;
                    if ends_statement(trimmed.get(end..).unwrap_or_default()) {
                        counts.code -= lines + 1;
                        counts.comment += lines + 1;
                        (state.mode, last_docstring) = (Mode::Code, offset);
                    } else {
                        state = self.scan(trimmed, state, &mut |_| {});
                    }
                    continue;
                }
                Mode::Str(_) | Mode::Template => {
                    counts.code += 1;
                    if DOCSTRINGS && let Mode::Str(s) = state.mode {
                        last_str = Some((line.as_ptr().addr() - content.as_ptr().addr(), s));
                    }
                    state = self.scan(trimmed, state, &mut |_| {});
                    continue;
                }
                Mode::Code if !self.starts_with(first) => {
                    if DOCSTRINGS && string_start(trimmed).is_some() {
                        let offset = line.as_ptr().addr() - content.as_ptr().addr();
                        if let Some(s) = self.docstring_at(trimmed)
                            && !self.continues(content, offset, last_str, last_docstring)
                        {
                            match s {
                                Docstring::Line => {
                                    counts.comment += 1;
                                    last_docstring = offset;
                                }
                                Docstring::Opens(s) => {
                                    counts.code += 1;
                                    (state.mode, docstring_lines) = (Mode::Str(s), Some(1));
                                }
                            }
                            continue;
                        }
                    }
                    counts.code += 1;
                    if needs_scan(line, &state) {
                        state = self.scan(trimmed, state, &mut |_| {});
                    }
                    continue;
                }
                Mode::Code | Mode::Block(_) => {}
            }
            // Delimiters such as `REM ` end in a space, so they are matched against the line
            // without its trailing whitespace.
            let trimmed = trim_end(trimmed);
            // The block comment that the rest of the line starts inside, if there is one.
            let (mut comment, rest) = if let Mode::Block(comment) = state.mode {
                (comment, trimmed)
            } else if let Some((rest, comment)) = style.block.iter().find_map(|&(open, close)| {
                let rest = trimmed.strip_prefix(open.as_bytes())?;
                Some((rest, Comment { open: open.as_bytes(), close: close.as_bytes(), depth: 0 }))
            }) {
                (comment, rest)
            } else if style.line.iter().any(|p| trimmed.starts_with(p.as_bytes())) {
                counts.comment += 1;
                continue;
            } else {
                counts.code += 1;
                if needs_scan(line, &state) {
                    state = self.scan(trimmed, state, &mut |_| {});
                }
                continue;
            };
            match self.comment_end(rest, &mut comment).and_then(|end| rest.get(end..)) {
                None => {
                    counts.comment += 1;
                    state.mode = Mode::Block(comment);
                }
                Some(after) => {
                    state.mode = Mode::Code;
                    if trim_start(after).is_empty() {
                        counts.comment += 1;
                    } else {
                        counts.code += 1;
                        if needs_scan(line, &state) {
                            state = self.scan(after, state, &mut |_| {});
                        }
                    }
                }
            }
        }
        counts
    }

    /// Count the lines from `line` on, which `lines` follow in `content`, while they start in code
    /// outside every string, end before the offset `until` and open no block comment. Return the
    /// first line that does not, or `None` after the last line.
    ///
    /// Such a line is blank, a line comment or code by its first bytes alone, and most lines are
    /// such lines. The loop keeps its cursor and counts in registers only when it is a function of
    /// its own, since the state of the loop in [`Syntax::count`] crowds them out.
    #[inline(never)]
    fn count_plain<'a, const PUSH: bool>(
        &self,
        content: &'a [u8],
        mut line: &'a [u8],
        lines: &mut Lines<'a>,
        until: usize,
        counts: &mut LineCounts,
        code_starts: &mut CodeStarts,
    ) -> Option<&'a [u8]> {
        let (mut rest, mut local) = (lines.clone(), *counts);
        let next = loop {
            if line.as_ptr().addr() - content.as_ptr().addr() + line.len() >= until {
                break Some(line);
            }
            let trimmed = trim_start(line);
            match trimmed.first() {
                None => local.blank += 1,
                Some(&first) if !self.starts_with(first) => {
                    if PUSH && first == b'#' {
                        code_starts.push((line.as_ptr().addr() - content.as_ptr().addr(), local));
                    }
                    local.code += 1;
                }
                Some(_) => match self.line_comment(trimmed) {
                    None => break Some(line),
                    Some(true) => local.comment += 1,
                    Some(false) => local.code += 1,
                },
            }
            let Some(next) = rest.next() else { break None };
            line = next;
        };
        (*lines, *counts) = (rest, local);
        next
    }

    /// Whether `text`, a line without leading whitespace that starts in code outside every string,
    /// is a line comment rather than code, or `None` if it opens a block comment.
    ///
    /// The loop in [`Syntax::count_plain`] keeps more of its state in registers when this is not
    /// inlined into it.
    #[inline(never)]
    fn line_comment(&self, text: &[u8]) -> Option<bool> {
        // Delimiters such as `REM ` end in a space, so they are matched against the line without
        // its trailing whitespace.
        let text = trim_end(text);
        if self.style.block.iter().any(|&(open, _)| text.starts_with(open.as_bytes())) {
            return None;
        }
        Some(self.style.line.iter().any(|p| text.starts_with(p.as_bytes())))
    }

    /// The offset in `text` after the end of `comment`, or `None` if it does not end in `text`, in
    /// which case `comment` holds its depth at the end of `text`.
    fn comment_end(&self, text: &[u8], comment: &mut Comment) -> Option<usize> {
        let mut i = 0;
        loop {
            let rest = text.get(i..)?;
            let close = find(rest, comment.close);
            let open = if self.nests { find(rest, comment.open) } else { None };
            match (open, close) {
                (Some(open), _) if close.is_none_or(|close| open < close) => {
                    comment.depth += 1;
                    i += open + comment.open.len();
                }
                (_, Some(close)) => {
                    i += close + comment.close.len();
                    let Some(depth) = comment.depth.checked_sub(1) else { return Some(i) };
                    comment.depth = depth;
                }
                (_, None) => return None,
            }
        }
    }

    /// The sorted offsets of the lines of `content` that may end inside a block comment that
    /// opens after code on them.
    ///
    /// Such a line has an opening delimiter with something before it and, unless comments nest,
    /// no close after it. Some lines found are only inside strings or comments, which costs time
    /// but not accuracy.
    fn opens_after_code(&self, content: &[u8]) -> Vec<usize> {
        let mut offsets = Vec::new();
        for &(open, close) in self.style.block {
            let (open, close) = (open.as_bytes(), close.as_bytes());
            for at in memchr::memmem::find_iter(content, open) {
                let Some((head, tail)) = content.split_at_checked(at) else { continue };
                // Most delimiters start their lines, which a look back over indentation finds.
                let before = head.iter().rev().find(|&&b| b != b' ' && b != b'\t');
                if matches!(before, None | Some(b'\n')) {
                    continue;
                }
                let head = line_start(head);
                let rest = tail.strip_prefix(open).map_or(tail, line_end);
                if !trim_start(head).is_empty() && (self.nests || find(rest, close).is_none()) {
                    offsets.push(at - head.len());
                }
            }
        }
        offsets.sort_unstable();
        offsets.dedup();
        offsets
    }

    /// The state after `text`, which starts in `state`, where `code` is called with each run of
    /// `text` that is outside every comment and string and character literal.
    ///
    /// Comments and strings are found in the order in which they open, so that a quote inside a
    /// comment, as in `// don't`, does not open a string, and a comment delimiter inside a string
    /// does not open a comment. A string that does not span lines ends with the line.
    ///
    /// Most lines of code are not scanned, and the loop in [`Syntax::count`] is faster when this
    /// is not inlined into it.
    #[inline(never)]
    fn scan<'a>(&self, text: &'a [u8], mut state: State, code: &mut impl FnMut(&'a [u8])) -> State {
        let mut i = 0;
        // The start of the run of code that `code` has not yet been called with.
        let mut start = 0;
        loop {
            match state.mode {
                Mode::Block(mut comment) => {
                    let Some(end) = text.get(i..).and_then(|t| self.comment_end(t, &mut comment))
                    else {
                        state.mode = Mode::Block(comment);
                        return state;
                    };
                    (i, start, state.mode) = (i + end, i + end, Mode::Code);
                }
                Mode::Str(s) => {
                    let Some(end) = s.end(text, i) else {
                        if !s.spans {
                            state.mode = Mode::Code;
                        }
                        return state;
                    };
                    (i, start, state.mode) = (end, end, Mode::Code);
                }
                Mode::Template => {
                    let Some(end) = template_end(text, i) else { return state };
                    i = end;
                    // The literal ends at a backtick, and a substitution opens at `${`.
                    if text.get(end - 1) == Some(&b'`') {
                        (start, state.mode) = (end, Mode::Code);
                    } else if state.substitutions >> 56 == 0 {
                        state.substitutions = (state.substitutions << 8) | 1;
                        (start, state.mode) = (end, Mode::Code);
                    }
                }
                Mode::Code => {
                    let Some(at) = text.get(i..).and_then(|t| self.find_special(t)) else {
                        code(text.get(start..).unwrap_or_default());
                        return state;
                    };
                    i += at;
                    let Some(rest @ &[byte, ..]) = text.get(i..) else { return state };
                    let run = text.get(start..i).unwrap_or_default();
                    if self.starts_with(byte) {
                        let block = self.style.block.iter().find_map(|&(open, close)| {
                            let (open, close) = (open.as_bytes(), close.as_bytes());
                            starts_with(rest, open).then_some(Comment { open, close, depth: 0 })
                        });
                        if let Some(comment) = block {
                            code(run);
                            (i, state.mode) = (i + comment.open.len(), Mode::Block(comment));
                            continue;
                        }
                        if self.style.line.iter().any(|p| starts_with(rest, p.as_bytes())) {
                            code(run);
                            return state;
                        }
                        if byte == b'/' && self.javascript && opens_regex(text, i) {
                            code(run);
                            i = regex_end(text, i);
                            start = i;
                            continue;
                        }
                    }
                    let innermost = state.substitutions & 0xFF;
                    match byte {
                        b'"' | b'\'' | b'`' | b'|' => {
                            if let Some((mode, contents)) = self.string_at(text, i) {
                                code(run);
                                (i, state.mode) = (contents, mode);
                                continue;
                            }
                            if byte == b'\''
                                && let Some(end) = char_literal_end(text, i)
                            {
                                code(run);
                                (i, start) = (end, end);
                                continue;
                            }
                        }
                        b'{' if !matches!(innermost, 0 | 0xFF) => state.substitutions += 1,
                        b'}' if innermost == 1 => {
                            code(run);
                            state.substitutions >>= 8;
                            state.mode = Mode::Template;
                        }
                        b'}' if innermost > 1 => state.substitutions -= 1,
                        _ => {}
                    }
                    i += 1;
                }
            }
        }
    }

    /// The docstring that `text`, a line without leading whitespace that starts in code, starts
    /// with, or `None` if it does not start with a string or the string does not end its statement
    /// on the line.
    ///
    /// The string may have a prefix of `r`, `u` or `b` in either case. An f-string is not a
    /// docstring.
    fn docstring_at(&self, text: &[u8]) -> Option<Docstring> {
        let Some((Mode::Str(s), contents)) = self.string_at(text, string_start(text)?) else {
            return None;
        };
        match s.end(text, contents) {
            Some(end) => ends_statement(text.get(end..)?).then_some(Docstring::Line),
            None => s.spans.then_some(Docstring::Opens(s)),
        }
    }

    /// Whether the line at `offset` in `content` continues the statement of the line of code
    /// before it, so that a string that starts the line is part of an expression.
    ///
    /// `last_str` is the offset of the last line before `offset` that starts inside a string, with
    /// that string, and `last_docstring` is the offset of the last line on which a docstring ends.
    /// The lines after `last_str` start outside every string.
    ///
    /// A statement continues when the line before ends with `\`, an operator, a comma, `.` or an
    /// opening bracket, or with `and`, `or`, `not`, `in`, `is`, `if` or `else`, or when it opens
    /// more brackets than it closes. It also continues when the line before is a string that is
    /// not a docstring, since two strings in brackets join into one.
    #[inline(never)]
    fn continues(
        &self,
        content: &[u8],
        offset: usize,
        last_str: Option<(usize, Str)>,
        last_docstring: usize,
    ) -> bool {
        let mut end = offset;
        let (start, line, state) = loop {
            let Some(head) = end.checked_sub(1).and_then(|end| content.get(..end)) else {
                return false;
            };
            let start = memchr::memrchr(b'\n', head).map_or(0, |n| n + 1);
            let line = head.get(start..).unwrap_or_default();
            if let Some((at, s)) = last_str
                && at == start
            {
                break (start, line, State { mode: Mode::Str(s), substitutions: 0 });
            }
            let trimmed = trim_start(line);
            if !trimmed.is_empty() && !trimmed.starts_with(b"#") {
                break (start, line, State::default());
            }
            end = start;
        };
        if start == last_docstring {
            return false;
        }
        let text = trim_start(line);
        // The end of the last run of code, before any comment, and the brackets that the code
        // opens less those that it closes.
        let (mut code_end, mut depth) = (0, 0i32);
        let base = text.as_ptr().addr();
        self.scan(text, state, &mut |run| {
            code_end = run.as_ptr().addr() - base + run.len();
            for &byte in run {
                depth += i32::from(matches!(byte, b'(' | b'[' | b'{'));
                depth -= i32::from(matches!(byte, b')' | b']' | b'}'));
            }
        });
        let code = trim_end(text.get(..code_end).unwrap_or_default());
        let word_start = code.iter().rposition(|&b| !(b.is_ascii_alphanumeric() || b == b'_'));
        let word = code.get(word_start.map_or(0, |i| i + 1)..).unwrap_or_default();
        match code.last() {
            _ if depth > 0 => true,
            None => false,
            Some(b'"' | b'\'') => state.mode == Mode::Code && string_start(text).is_some(),
            Some(byte) if b"\\([{,.=+-*/%&|^<>~@".contains(byte) => true,
            Some(_) => matches!(word, b"and" | b"or" | b"not" | b"in" | b"is" | b"if" | b"else"),
        }
    }

    /// The mode inside the string that the quote at `at` in `code` opens, and the offset of its
    /// contents, or `None` if the quote does not open one.
    fn string_at(&self, code: &[u8], at: usize) -> Option<(Mode, usize)> {
        const HASHES: &[u8] = b"################";
        let (head, rest) = code.split_at_checked(at)?;
        let hashes = head.iter().rev().take_while(|&&b| b == b'#').count();
        let rust_raw = || {
            let r = head.get(..head.len() - hashes).and_then(|h| h.strip_suffix(b"r"));
            let start = r.map(|r| r.strip_suffix(b"b").or(r.strip_suffix(b"c")).unwrap_or(r));
            start.is_some_and(|start| !ident_before(start, start.len()))
        };
        self.literals.iter().find_map(|literal| {
            let open = starts_with(rest, literal.open).then_some(literal.open)?;
            let (quotes, hashes) = match literal.special {
                Plain => (open, 0),
                After(prefix) if head.ends_with(prefix) => (open, 0),
                UnlessAfter(prefixes) if !prefixes.iter().any(|p| head.ends_with(p)) => (open, 0),
                RustRaw if rust_raw() => (open, hashes),
                Hashes if hashes > 0 => (open, hashes),
                Run => (rest.get(..rest.iter().take_while(|&&b| b == b'"').count())?, 0),
                CppRaw => return cpp_raw_string_at(code, at),
                Template => return Some((Mode::Template, at + 1)),
                After(_) | UnlessAfter(_) | RustRaw | Hashes => return None,
            };
            let s = Str::new(&[quotes, HASHES.get(..hashes)?], literal.escape, literal.spans)?;
            Some((Mode::Str(s), at + quotes.len()))
        })
    }
}

/// A string at the start of a line that is a statement of its own.
enum Docstring {
    /// The string ends on the line, and nothing but a comment follows it.
    Line,
    /// The string continues on the next line, and is a docstring if its statement ends where it
    /// does.
    Opens(Str),
}

/// The offset of the quote that opens the string at the start of `text`, after a prefix of `r`, `u`
/// or `b` in either case, or `None` if `text` does not start with such a string.
fn string_start(text: &[u8]) -> Option<usize> {
    match text {
        [b'"' | b'\'', ..] => Some(0),
        [b'r' | b'R' | b'u' | b'U' | b'b' | b'B', b'"' | b'\'', ..] => Some(1),
        _ => None,
    }
}

/// Whether `rest`, the text after a string, ends the statement that the string starts, which it
/// does when it is blank or a comment.
fn ends_statement(rest: &[u8]) -> bool {
    matches!(trim_start(rest).first(), None | Some(b'#'))
}

/// Whether the `/` at `slash` in the JavaScript `code` opens a regular expression rather than
/// dividing.
///
/// A regular expression follows an operator, an opening bracket, `=>`, a keyword such as
/// `return`, or nothing. A `/` after any other `>`, or after `<` or `}`, is taken for division,
/// or for the end of a JSX tag such as `</a>` or `<b />`, because a regular expression rarely
/// follows them.
fn opens_regex(code: &[u8], slash: usize) -> bool {
    const KEYWORDS: &[&[u8]] = &[
        b"return", b"typeof", b"case", b"in", b"of", b"delete", b"void", b"throw", b"new", b"else",
        b"do", b"yield", b"await",
    ];
    let head = trim_end(code.get(..slash).unwrap_or_default());
    match head.last() {
        None => true,
        Some(b'(' | b',' | b'=' | b':' | b'[' | b'!' | b'&' | b'|' | b'?' | b'{' | b';' | b'~') => {
            true
        }
        Some(b'+' | b'-' | b'*' | b'%' | b'^') => true,
        Some(b'>') => head.ends_with(b"=>"),
        Some(_) => {
            let word = head.iter().rev().take_while(|&&b| b.is_ascii_alphanumeric() || b == b'_');
            let start = head.len() - word.count();
            !ident_before(head, start) && KEYWORDS.contains(&head.get(start..).unwrap_or_default())
        }
    }
}

/// The offset after the JavaScript regular expression whose opening `/` is at `slash` in `code`,
/// or the end of `code` if it is not closed there. A `/` inside a character class such as `[/]`
/// does not close it.
fn regex_end(code: &[u8], slash: usize) -> usize {
    let mut i = slash + 1;
    let mut in_class = false;
    while let Some(&byte) = code.get(i) {
        match byte {
            b'\\' => i += 1,
            b'[' => in_class = true,
            b']' => in_class = false,
            b'/' if !in_class => return i + 1,
            _ => {}
        }
        i += 1;
    }
    code.len()
}

/// The offset of the first of `bytes` in `content` at or after `from`, or `usize::MAX` if there is
/// none.
fn find_any(content: &[u8], from: usize, bytes: &[u8]) -> usize {
    let Some(rest) = content.get(from..) else { return usize::MAX };
    let at = match *bytes {
        [a] => memchr::memchr(a, rest),
        [a, b] => memchr::memchr2(a, b, rest),
        [a, b, c] => memchr::memchr3(a, b, c, rest),
        _ => None,
    };
    at.map_or(usize::MAX, |at| from + at)
}

/// `text` up to its first newline.
fn line_end(text: &[u8]) -> &[u8] {
    match memchr::memchr(b'\n', text).and_then(|n| text.split_at_checked(n)) {
        Some((line, _)) => line,
        None => text,
    }
}

/// `text` after its last newline.
fn line_start(text: &[u8]) -> &[u8] {
    match memchr::memrchr(b'\n', text).and_then(|n| text.get(n + 1..)) {
        Some(line) => line,
        None => text,
    }
}

/// Mark `byte` in `table`, which has an entry for every byte.
fn mark(table: &mut [bool; 256], byte: u8) {
    if let Some(entry) = table.get_mut(usize::from(byte)) {
        *entry = true;
    }
}

/// The offset of the first `delimiter` in `text`.
fn find(text: &[u8], delimiter: &[u8]) -> Option<usize> {
    // Delimiters are a few bytes, so finding the first byte and comparing the rest is faster than
    // building a substring searcher for each line or file.
    let Some((&first, rest)) = delimiter.split_first() else { return Some(0) };
    let mut from = 0;
    while let Some(i) = text.get(from..).and_then(|tail| memchr::memchr(first, tail)) {
        let at = from + i + 1;
        if text.get(at..).is_some_and(|tail| starts_with(tail, rest)) {
            return Some(at - 1);
        }
        from = at;
    }
    None
}

/// Whether `text` starts with `prefix`, a delimiter or quote of a few bytes, which are compared
/// one by one, since that is faster than a call to `memcmp` for so few.
fn starts_with(text: &[u8], prefix: &[u8]) -> bool {
    text.len() >= prefix.len() && text.iter().zip(prefix).all(|(a, b)| a == b)
}

/// The lines of `content`, split as [`str::lines`] splits them, except that a line keeps a
/// trailing `\r`, which [`trim`] removes.
pub fn lines(content: &[u8]) -> Lines<'_> {
    Lines { rest: Some(content) }
}

/// The iterator that [`lines`] returns.
#[derive(Clone)]
pub struct Lines<'a> {
    /// The text after the last line returned, or `None` after the last line.
    rest: Option<&'a [u8]>,
}

impl<'a> Iterator for Lines<'a> {
    type Item = &'a [u8];

    // Counting spends much of its time here, and the loops that call this are fastest with it
    // inlined.
    #[inline(always)]
    fn next(&mut self) -> Option<&'a [u8]> {
        // The text after a final newline is not a line.
        let rest = self.rest.filter(|rest| !rest.is_empty())?;
        let (line, rest) = match memchr::memchr(b'\n', rest).and_then(|n| rest.split_at_checked(n))
        {
            Some((line, newline)) => (line, newline.get(1..)),
            None => (rest, None),
        };
        self.rest = rest;
        Some(line)
    }
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
            0x80.. => match leading_whitespace_char(text) {
                Some(len) => len,
                None => break,
            },
            _ => break,
        };
        let Some(rest) = text.get(len..) else { break };
        text = rest;
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
                let continuations =
                    text.iter().rev().take(4).position(|&b| b & 0xC0 != 0x80).unwrap_or(0);
                let start = text.len() - 1 - continuations;
                match text.get(start..).and_then(whitespace_char) {
                    Some(len) => len,
                    None => break,
                }
            }
            _ => break,
        };
        let Some(rest) = text.len().checked_sub(len).and_then(|end| text.get(..end)) else {
            break;
        };
        text = rest;
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

/// The length of the whitespace character that starts `text`, if it starts with one.
///
/// A line rarely starts with a byte outside ASCII, and the loops that trim lines keep fewer
/// registers busy when this is not inlined into them.
#[cold]
#[inline(never)]
fn leading_whitespace_char(text: &[u8]) -> Option<usize> {
    text.get(..utf8_width(*text.first()?)).and_then(whitespace_char)
}

/// The length of `bytes` if they are exactly one whitespace character.
fn whitespace_char(bytes: &[u8]) -> Option<usize> {
    let mut chars = std::str::from_utf8(bytes).ok()?.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_whitespace() => Some(bytes.len()),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
