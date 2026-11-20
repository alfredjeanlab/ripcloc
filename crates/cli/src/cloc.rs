// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! Line counting with per-language comment detection.
//!
//! Comments are found line by line with simple heuristics rather than parsing, as `cloc` does.

use std::borrow::Cow;

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
        // A `.pp` file is Puppet rather than Free Pascal, as in tokei and scc, since Free Pascal
        // units are mostly named `.pas`.
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
        _ => None,
    }
}

/// Count the lines of `content`, using the comment syntax for the extension `ext`.
///
/// A line with code is code, even when a block comment opens or closes on it, and the lines
/// after it inside the comment are comments. Every non-blank line inside a string literal is code,
/// even when it looks like a comment, and so is a Python docstring. Without a known comment
/// syntax, every non-blank line is code. The content need not be UTF-8, and a leading UTF-8 byte
/// order mark is ignored.
///
/// In HTML, Vue and Svelte, the lines inside a `<script>` or `<style>` element are counted in the
/// language of the element, and the lines that open and close it are markup.
pub fn count_lines(content: &(impl AsRef<[u8]> + ?Sized), ext: &str) -> LineCounts {
    let content = content.as_ref();
    let content = content.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(content);
    if matches!(ext, "html" | "htm" | "vue" | "svelte") {
        return count_markup(content, ext);
    }
    count_in(content, ext)
}

/// Count the lines of `content`, which has no byte order mark, in the language of `ext` alone.
fn count_in(content: &[u8], ext: &str) -> LineCounts {
    let syntax = Syntax::new(ext);
    let opens_after_code =
        if syntax.c_like { syntax.opens_after_code(content) } else { Vec::new() };
    syntax.count(content, &opens_after_code)
}

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
            counts.add(count_in(content.get(body..at).unwrap_or_default(), language));
            (markup, element) = (at, None);
            tags = line.get(end + close.len()..).unwrap_or_default();
        }
        if let Some((close, language)) = element_opening(tags, &mut in_comment) {
            let body = (at + line.len() + 1).min(content.len());
            counts.add(count_in(content.get(markup..body).unwrap_or_default(), ext));
            element = Some((close, language, body));
        }
    }
    let (rest, language) = match element {
        Some((_, language, body)) => (body, language),
        None => (markup, ext),
    };
    counts.add(count_in(content.get(rest..).unwrap_or_default(), language));
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

/// The lines of `content` in the language of `ext`, each with its code, which is the line without
/// its comments and without its string and character literals.
pub fn code_lines<'a>(
    content: &'a [u8],
    ext: &str,
) -> impl Iterator<Item = (&'a [u8], Cow<'a, [u8]>)> {
    let (syntax, mut state) = (Syntax::new(ext), State::default());
    lines(content).map(move |line| {
        let mut code = Cow::Borrowed(&[][..]);
        state = syntax.scan(line, state, &mut |run| {
            if code.is_empty() {
                code = Cow::Borrowed(run);
            } else if !run.is_empty() {
                code.to_mut().extend_from_slice(run);
            }
        });
        (line, code)
    })
}

/// A kind of string literal, which opens at the quotes `open` and closes at the same quotes.
#[derive(Clone, Copy)]
struct Literal {
    open: &'static [u8],
    /// The byte that escapes the byte after it, or 0 if there is none. A quote that is its own
    /// escape stands for itself when it is doubled.
    escape: u8,
    /// Whether a literal that is not closed on its line continues on the next.
    spans: bool,
    special: Special,
}

/// A rule for the literals that the other fields of a [`Literal`] cannot describe.
#[derive(Clone, Copy)]
enum Special {
    Plain,
    /// The quotes follow these bytes, as in C#'s verbatim string `@"..."`.
    After(&'static [u8]),
    /// The quotes follow `r`, `br` or `cr` and any number of `#`, as in Rust's `r#"..."#`, and the
    /// literal closes at the quotes and as many `#`. An `r` that ends an identifier does not count.
    RustRaw,
    /// The quotes follow one or more `#`, as in Swift's `#"..."#`, and the literal closes at the
    /// quotes and as many `#`.
    Hashes,
    /// The quote follows `R`, as in C++'s `R"x(...)x"`, which closes at `)`, its delimiter and `"`.
    CppRaw,
    /// The literal closes at as many quotes as the run that opens it, as in C#'s `""""...""""`.
    Run,
    /// A JavaScript template literal, whose `${...}` substitutions hold code.
    Template,
}

use Special::*;

const fn lit(open: &'static [u8], escape: u8, spans: bool, special: Special) -> Literal {
    Literal { open, escape, spans, special }
}

/// C's strings, which have backslash escapes and do not span lines.
const QUOTED: Literal = lit(b"\"", b'\\', false, Plain);
const TRIPLE: Literal = lit(b"\"\"\"", b'\\', true, Plain);
const RUST: &[Literal] = &[lit(b"\"", 0, true, RustRaw), lit(b"\"", b'\\', true, Plain)];
const GO: &[Literal] = &[lit(b"`", 0, true, Plain), QUOTED];
const JAVASCRIPT: &[Literal] =
    &[lit(b"`", 0, true, Template), QUOTED, lit(b"'", b'\\', false, Plain)];
/// A prefix such as `r` or `b` does not change where a Python string ends, because even a raw
/// string does not end at a quote after a backslash.
const PYTHON: &[Literal] =
    &[TRIPLE, lit(b"'''", b'\\', true, Plain), QUOTED, lit(b"'", b'\\', false, Plain)];
const CPP: &[Literal] = &[lit(b"\"", 0, true, CppRaw), QUOTED];
const CSHARP: &[Literal] = &[
    lit(b"\"", b'"', true, After(b"@")),
    lit(b"\"", b'"', true, After(b"@$")),
    lit(b"\"\"\"", 0, true, Run),
    QUOTED,
];
const JAVA: &[Literal] = &[TRIPLE, QUOTED];
const KOTLIN: &[Literal] = &[lit(b"\"\"\"", 0, true, Plain), QUOTED];
const SWIFT: &[Literal] =
    &[lit(b"\"\"\"", 0, true, Hashes), lit(b"\"", 0, false, Hashes), TRIPLE, QUOTED];
/// Strings that span lines and have backslash escapes, as in OCaml, Julia and Lean.
const SPANNING: Literal = lit(b"\"", b'\\', true, Plain);
/// Elixir's heredocs hold documentation, whose Markdown headings start with `#`. A `"` does not
/// span lines here, because `?"` is a character and not the start of a string.
const ELIXIR: &[Literal] = &[TRIPLE, lit(b"'''", b'\\', true, Plain), QUOTED];
const JULIA: &[Literal] = &[TRIPLE, SPANNING];
/// Nix's indented strings `''...''` hold shell scripts, whose `#` lines are code.
const NIX: &[Literal] = &[lit(b"''", 0, true, Plain), SPANNING];
const SOLIDITY: &[Literal] = &[QUOTED, lit(b"'", b'\\', false, Plain)];
/// Strings that span lines and stand for a quote with two quotes, as in Visual Basic and Coq.
const DOUBLED: &[Literal] = &[lit(b"\"", b'"', true, Plain)];
const TOML: &[Literal] = &[TRIPLE, lit(b"'''", 0, true, Plain), QUOTED, lit(b"'", 0, false, Plain)];

/// The string literals of the language of `ext`, in the order in which they are tried. A `'` that
/// does not open one of them starts a character literal or, in Rust, a lifetime.
fn literals(ext: &str) -> &'static [Literal] {
    match ext {
        "rs" => RUST,
        "go" => GO,
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "mts" | "cjs" | "cts" => JAVASCRIPT,
        // Dart, Groovy and Starlark have the same quotes and escapes as Python.
        "py" | "dart" | "gradle" | "bzl" | "bazel" | "bzlmod" | "star" => PYTHON,
        "ex" | "exs" => ELIXIR,
        "jl" => JULIA,
        "nix" => NIX,
        // Puppet has the same quotes as Solidity.
        "sol" | "pp" => SOLIDITY,
        "vb" | "coq" => DOUBLED,
        "hs" | "zig" | "zon" => &[QUOTED],
        "ml" | "mli" | "lean" => &[SPANNING],
        // A `.h` file may be C++, and `R"(` does not start a string in C either way.
        "cpp" | "hpp" | "cc" | "cxx" | "hh" | "h" | "mm" => CPP,
        "cs" => CSHARP,
        "java" => JAVA,
        "kt" | "kts" | "scala" => KOTLIN,
        "swift" => SWIFT,
        "toml" => TOML,
        _ => &[],
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
                _ if tail.starts_with(close) => return Some(at + close.len()),
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
    /// The quotes that can open a string that spans lines.
    spanning: Vec<u8>,
    /// Whether a block comment can open after code, as C's `/*` and OCaml's `(*` do. An HTML
    /// comment cannot, since a Markdown code block may show one opening.
    c_like: bool,
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
        for delimiter in delimiters {
            if let Some(&first) = delimiter.as_bytes().first() {
                mark(&mut starts, first);
            }
        }
        let mut special = starts;
        let quotes = literals.iter().filter_map(|l| l.open.first().copied());
        let braces = if javascript { b"{}".as_slice() } else { b"" };
        let char_quote = if literals.is_empty() { b"".as_slice() } else { b"'" };
        for byte in quotes.chain(braces.iter().copied()).chain(char_quote.iter().copied()) {
            mark(&mut special, byte);
        }
        let spanning = b"\"'`"
            .iter()
            .copied()
            .filter(|&q| literals.iter().any(|l| l.spans && l.open.first() == Some(&q)))
            .collect();
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
        );
        Self { style, literals, nests, javascript, starts, special, spanning, c_like }
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

    /// Count the lines of `content`, where a block comment can open after code only on the lines
    /// that start at the sorted offsets `opens_after_code`.
    fn count(&self, content: &[u8], opens_after_code: &[usize]) -> LineCounts {
        let style = &self.style;
        let mut counts = LineCounts::default();
        let mut state = State::default();
        let offset = |line: &[u8]| line.as_ptr().addr() - content.as_ptr().addr();
        let mut offsets = opens_after_code.iter().copied().peekable();
        let mut may_open = |line: &[u8]| {
            let start = offset(line);
            while offsets.next_if(|&offset| offset < start).is_some() {}
            offsets.peek() == Some(&start)
        };
        // The offset of the first byte at or after the cursor that can open a string that spans
        // lines. Finding it once for many lines keeps the cost of strings to the lines with one.
        let spanning = self.spanning.as_slice();
        let mut next_opener = find_any(content, 0, spanning);
        let mut has_opener = |line: &[u8]| {
            let start = offset(line);
            if next_opener < start {
                next_opener = find_any(content, start, spanning);
            }
            next_opener < start + line.len()
        };
        // The state after `text`, the rest of `line` from a point outside every comment and
        // string, where `state` holds any substitutions that the point is inside.
        let mut after_code = |line: &[u8], text: &[u8], state: State| {
            let may_open = !opens_after_code.is_empty() && may_open(line);
            if state.substitutions != 0 || has_opener(line) || may_open {
                self.scan(text, state, &mut |_| {})
            } else {
                state
            }
        };

        for line in lines(content) {
            let trimmed = trim_start(line);
            let Some(&first) = trimmed.first() else {
                counts.blank += 1;
                continue;
            };
            match state.mode {
                Mode::Str(_) | Mode::Template => {
                    counts.code += 1;
                    state = self.scan(trimmed, state, &mut |_| {});
                    continue;
                }
                Mode::Code if !self.starts_with(first) => {
                    counts.code += 1;
                    state = after_code(line, trimmed, state);
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
                state = after_code(line, trimmed, state);
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
                        state = after_code(line, after, state);
                    }
                }
            }
        }
        counts
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
                    let Some(at) =
                        text.get(i..).and_then(|t| t.iter().position(|&b| self.is_special(b)))
                    else {
                        code(text.get(start..).unwrap_or_default());
                        return state;
                    };
                    i += at;
                    let Some(rest @ &[byte, ..]) = text.get(i..) else { return state };
                    let run = text.get(start..i).unwrap_or_default();
                    if self.starts_with(byte) {
                        let block = self.style.block.iter().find_map(|&(open, close)| {
                            let (open, close) = (open.as_bytes(), close.as_bytes());
                            rest.starts_with(open).then_some(Comment { open, close, depth: 0 })
                        });
                        if let Some(comment) = block {
                            code(run);
                            (i, state.mode) = (i + comment.open.len(), Mode::Block(comment));
                            continue;
                        }
                        if self.style.line.iter().any(|p| rest.starts_with(p.as_bytes())) {
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
                        b'"' | b'\'' | b'`' => {
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
            let open = rest.starts_with(literal.open).then_some(literal.open)?;
            let (quotes, hashes) = match literal.special {
                Plain => (open, 0),
                After(prefix) if head.ends_with(prefix) => (open, 0),
                RustRaw if rust_raw() => (open, hashes),
                Hashes if hashes > 0 => (open, hashes),
                Run => (rest.get(..rest.iter().take_while(|&&b| b == b'"').count())?, 0),
                CppRaw => return cpp_raw_string_at(code, at),
                Template => return Some((Mode::Template, at + 1)),
                After(_) | RustRaw | Hashes => return None,
            };
            let s = Str::new(&[quotes, HASHES.get(..hashes)?], literal.escape, literal.spans)?;
            Some((Mode::Str(s), at + quotes.len()))
        })
    }
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
        if text.get(at..).is_some_and(|tail| tail.starts_with(rest)) {
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
        "xml" | "csproj" | "vbproj" | "fsproj" | "props" | "targets" | "xaml" | "resx"
        | "nuspec" => "XML",
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
        "zig" | "zon" => "Zig",
        "hs" => "Haskell",
        "ex" | "exs" => "Elixir",
        "ml" | "mli" => "OCaml",
        "dart" => "Dart",
        "jl" => "Julia",
        "clj" | "cljs" | "cljc" => "Clojure",
        "erl" | "hrl" => "Erlang",
        "nix" => "Nix",
        "tf" | "tfvars" => "Terraform",
        "lean" => "Lean",
        "mdx" => "MDX",
        "sol" => "Solidity",
        "gradle" => "Gradle",
        "vb" => "Visual Basic",
        "adoc" | "asciidoc" => "AsciiDoc",
        "bzl" | "bazel" | "bzlmod" | "star" => "Bazel",
        "pp" => "Puppet",
        "j2" | "jinja" | "jinja2" => "Jinja",
        "v" | "vh" => "Verilog",
        "sv" | "svh" => "SystemVerilog",
        "coq" => "Coq",
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
        "BUILD" | "WORKSPACE" => "bazel".to_string(),
        _ => path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase(),
    }
}

/// The key for a file without an extension whose content starts with `content`, from the
/// interpreter that its `#!` line names, such as `#!/usr/bin/env python3`.
///
/// A version after the interpreter's name is ignored, so `python3.12` is Python, and `env`'s
/// options and variable assignments are skipped.
pub fn shebang_key(content: &[u8]) -> Option<&'static str> {
    let rest = content.strip_prefix(b"#!")?;
    let line = rest.split(|&b| b == b'\n').next()?;
    let line = std::str::from_utf8(line).ok()?;
    let mut words = line.split_whitespace();
    let mut program = words.next()?.rsplit('/').next()?;
    if program == "env" {
        program = words.find(|w| !w.starts_with('-') && !w.contains('='))?.rsplit('/').next()?;
    }
    let name = program.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
    Some(match name {
        "python" | "pypy" => "py",
        "sh" | "ash" | "dash" | "ksh" | "mksh" => "sh",
        "bash" => "bash",
        "zsh" => "zsh",
        "fish" => "fish",
        "node" | "nodejs" | "deno" | "bun" => "js",
        "ruby" => "rb",
        "perl" => "pl",
        "php" => "php",
        "lua" | "luajit" => "lua",
        "awk" | "gawk" | "mawk" | "nawk" => "awk",
        "Rscript" => "r",
        "pwsh" => "ps1",
        "elixir" => "ex",
        "escript" => "erl",
        "julia" => "jl",
        "runghc" | "runhaskell" => "hs",
        "ocaml" => "ml",
        _ => return None,
    })
}

/// The key of a `.v` file whose content starts with `content`, which is `coq` for Coq and `v` for
/// Verilog.
///
/// The start of the file is scored for each language, and Verilog must score higher to win.
/// Keywords are whole words and case-sensitive, so `Module` is Coq and `module` is Verilog. A
/// keyword that is distinctive, such as `Lemma` or `endmodule`, weighs more than one that is
/// common, such as `End` or `begin`, so that a stray word does not decide. A line that ends in `.`
/// after a capitalized command is a Coq sentence, and a line that ends in `;` is a Verilog
/// statement.
///
/// The text inside comments and strings is skipped with one search for its end, since a comment
/// can mention a keyword of either language. Each kind of comment scores only once, because a long
/// license header is no stronger a sign than one line, and because Verilog writes attributes such
/// as `(* keep *)` in the same way that Coq writes comments.
///
/// Scanning stops at the end of the first line after which the scores differ by [`V_MARGIN`], so a
/// typical file is decided in its first few lines of code, and it never goes past [`V_PREFIX`].
///
/// A tie, which includes a file with no signal at all, such as an empty one, goes to Coq, because
/// tokei counts every `.v` file as Coq.
pub fn v_key(content: &[u8]) -> &'static str {
    let start = content.get(..V_PREFIX).unwrap_or(content);
    let (mut coq, mut verilog) = (0u32, 0u32);
    // Whether a `//`, `/*` or `(*` comment has been scored.
    let (mut line_comment, mut c_comment, mut ml_comment) = (false, false, false);
    // The first word of the current line's code, and its last byte that is not whitespace.
    let (mut first_word, mut last): (Option<&[u8]>, Option<u8>) = (None, None);
    let mut i = 0;
    // Score the end of a line, whose first word and last byte that is not whitespace are given.
    let end_line =
        |first_word: Option<&[u8]>, last: Option<u8>, coq: &mut u32, verilog: &mut u32| {
            let command = first_word.and_then(<[u8]>::first).is_some_and(u8::is_ascii_uppercase);
            match last {
                Some(b';') => *verilog += 1,
                Some(b'.') if command => *coq += 1,
                _ => {}
            }
        };
    while let Some(rest) = start.get(i..).filter(|rest| !rest.is_empty()) {
        let (score, seen, close): (_, _, &[u8]) = match rest {
            [b'\n', ..] => {
                end_line(first_word, last, &mut coq, &mut verilog);
                if coq.abs_diff(verilog) >= V_MARGIN {
                    return if verilog > coq { "v" } else { "coq" };
                }
                (first_word, last, i) = (None, None, i + 1);
                continue;
            }
            [b'/', b'/', ..] => (&mut verilog, &mut line_comment, b"\n"),
            [b'/', b'*', ..] => (&mut verilog, &mut c_comment, b"*/"),
            [b'(', b'*', ..] => (&mut coq, &mut ml_comment, b"*)"),
            _ => {
                i += v_token(rest, &mut coq, &mut verilog, &mut first_word, &mut last);
                continue;
            }
        };
        *score += u32::from(!*seen);
        *seen = true;
        // A line comment ends before its newline, which ends the line of code before it.
        let body = rest.get(2..).unwrap_or_default();
        i += 2 + match close {
            b"\n" => memchr::memchr(b'\n', body).unwrap_or(body.len()),
            _ => memchr::memmem::find(body, close).map_or(body.len(), |at| at + close.len()),
        };
    }
    end_line(first_word, last, &mut coq, &mut verilog);
    if verilog > coq { "v" } else { "coq" }
}

/// How much one language must outscore the other at the end of a line for [`v_key`] to stop.
const V_MARGIN: u32 = 8;

/// Score the token at the start of `code`, which is outside every comment, and return its length.
/// `first_word` and `last` are the line's first word and its last byte that is not whitespace,
/// which are updated with the token.
fn v_token<'a>(
    code: &'a [u8],
    coq: &mut u32,
    verilog: &mut u32,
    first_word: &mut Option<&'a [u8]>,
    last: &mut Option<u8>,
) -> usize {
    let is_word = |b: &u8| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'\'';
    let Some(&byte) = code.first() else { return 1 };
    match byte {
        b'"' => {
            // A string is taken to end with its line, so that the line's end is still scored.
            let body = code.get(1..).unwrap_or_default();
            let len = match memchr::memchr2(b'"', b'\n', body) {
                Some(n) if body.get(n) == Some(&b'"') => n + 2,
                Some(n) => n + 1,
                None => code.len(),
            };
            *last = Some(b'"');
            len
        }
        b'`' => {
            let len = 1 + code.get(1..).map_or(0, |s| s.iter().take_while(|b| is_word(b)).count());
            let directive = code.get(1..len).unwrap_or_default();
            if matches!(directive, b"timescale" | b"include" | b"define" | b"ifdef" | b"ifndef") {
                *verilog += 3;
            }
            *last = Some(b'`');
            len
        }
        b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
            let len = code.iter().take_while(|b| is_word(b)).count();
            let word = code.get(..len).unwrap_or_default();
            let dot = code.get(len) == Some(&b'.');
            let (coq_weight, verilog_weight) = match word {
                b"Proof" | b"Qed" | b"Defined" | b"Admitted" if dot => (3, 0),
                b"Require" | b"Theorem" | b"Lemma" | b"Definition" | b"Fixpoint" | b"Inductive" => {
                    (3, 0)
                }
                b"Module" | b"Section" | b"Variable" | b"Hypothesis" | b"Implicit" => (2, 0),
                b"Import" | b"Export" | b"From" | b"End" | b"Set" | b"Arguments" => (1, 0),
                b"module" | b"endmodule" | b"posedge" | b"negedge" => (0, 3),
                b"always" | b"assign" | b"wire" | b"reg" | b"input" | b"output" => (0, 2),
                b"begin" | b"end" | b"parameter" | b"localparam" | b"initial" => (0, 1),
                _ => (0, 0),
            };
            (*coq, *verilog) = (*coq + coq_weight, *verilog + verilog_weight);
            first_word.get_or_insert(word);
            *last = word.last().copied();
            len.max(1)
        }
        _ => {
            if !byte.is_ascii_whitespace() {
                *last = Some(byte);
            }
            1
        }
    }
}

/// The most bytes at the start of a `.v` file that are read to tell Coq from Verilog.
///
/// A license header and the code after it fit, since a header of a few kilobytes is skipped with
/// one search for its end. The bound is that of the binary check in `main.rs`, which already reads
/// these bytes, so the worst case costs about as much as that check.
const V_PREFIX: usize = 8000;

/// Whether files with the key `key` are split into source and tests. Documentation, data,
/// configuration and build files are not.
pub fn is_split(key: &str) -> bool {
    !matches!(
        key,
        "md" | "markdown"
            | "mdx"
            | "rst"
            | "adoc"
            | "asciidoc"
            | "tex"
            | "sty"
            | "yaml"
            | "yml"
            | "toml"
            | "json"
            | "make"
            | "dockerfile"
            | "gradle"
            | "bzl"
            | "bazel"
            | "bzlmod"
            | "star"
            | "tfvars"
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
    fn python_triple_quoted_strings_are_code() {
        // A docstring is code, as in tokei.
        let content = "def f():\n    \"\"\"Doc.\n    # in\n    \"\"\"\n    # c\n";
        assert_eq!(counts(content, "py"), (0, 1, 4));
        let content = "'''Module.\n# in\n'''\n";
        assert_eq!(counts(content, "py"), (0, 0, 3));
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
        let content =
            "const a = b / 2 + `\n// in\n` / c;\n// c\nconst d = <a href={`/`}></a>;\n// e\n";
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
    fn leading_byte_order_mark_is_ignored() {
        let content = b"\xEF\xBB\xBF// bom\nint x;\n";
        assert_eq!(count_lines(content, "c"), LineCounts { blank: 0, comment: 1, code: 1 });
        assert_eq!(
            count_lines(b"\xEF\xBB\xBF\n", "c"),
            LineCounts { blank: 1, comment: 0, code: 0 }
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
        assert!(!is_counted("svg") && !is_counted("po"));
    }

    #[test]
    fn documentation_data_and_build_configuration_are_not_split() {
        for key in ["rst", "tex", "sty", "m4", "autoconf", "automake", "ld", "lds", "json", "yaml"]
        {
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
        let content =
            "Definition p := \"C:\\\".\n(* c *)\nDefinition s := \"a \"\"\n(* in *)\n\".\n";
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
        let verilog =
            format!("{}`timescale 1ns/1ps\nmodule m;\n", "// SPDX and more\n".repeat(200));
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
}
