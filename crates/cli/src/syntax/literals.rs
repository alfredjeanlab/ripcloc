// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! The string literals of each language, which the scanner skips, so that a comment marker inside
//! one does not start a comment.

/// A kind of string literal, which opens at the quotes `open` and closes at the same quotes.
#[derive(Clone, Copy)]
pub(super) struct Literal {
    pub(super) open: &'static [u8],
    /// The byte that escapes the byte after it, or 0 if there is none. A quote that is its own
    /// escape stands for itself when it is doubled.
    pub(super) escape: u8,
    /// Whether a literal that is not closed on its line continues on the next.
    pub(super) spans: bool,
    pub(super) special: Special,
}

/// A rule for the literals that the other fields of a [`Literal`] cannot describe.
#[derive(Clone, Copy)]
pub(super) enum Special {
    Plain,
    /// The quotes follow these bytes, as in C#'s verbatim string `@"..."`.
    After(&'static [u8]),
    /// The quotes do not follow any of these, which make them a character, as in Scheme's `#\"`.
    UnlessAfter(&'static [&'static [u8]]),
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
pub(super) const QUOTED: Literal = lit(b"\"", b'\\', false, Plain);
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
/// F#'s triple-quoted strings have no escapes, and its verbatim strings double their quotes.
const FSHARP: &[Literal] = &[
    lit(b"\"\"\"", 0, true, Plain),
    lit(b"\"", b'"', true, After(b"@")),
    lit(b"\"", b'"', true, After(b"$@")),
    lit(b"\"", b'"', true, After(b"@$")),
    SPANNING,
];
/// Triple-quoted strings without escapes, as in PureScript and Nim, beside C's strings.
const RAW_TRIPLE: &[Literal] = &[lit(b"\"\"\"", 0, true, Plain), QUOTED];
/// VHDL's strings double their quotes and do not span lines.
const VHDL: &[Literal] = &[lit(b"\"", b'"', false, Plain)];
/// Pascal's strings double their quotes, so that `'{'` does not open a comment.
const PASCAL: &[Literal] = &[lit(b"'", b'\'', false, Plain)];
/// D's WYSIWYG strings `r"..."` and `` `...` `` have no escapes. A delimited string such as
/// `q"(...)"` is taken for a plain string, which ends at the same quote unless it holds one.
const D: &[Literal] = &[lit(b"\"", 0, true, After(b"r")), lit(b"`", 0, true, Plain), SPANNING];
/// In Emacs Lisp, `?"` and `?\"` are characters.
const EMACS_LISP: &[Literal] = &[lit(b"\"", b'\\', true, UnlessAfter(&[b"?", b"?\\"]))];
/// In Scheme, Racket and Common Lisp, `#\"` is a character.
const LISP: &[Literal] = &[lit(b"\"", b'\\', true, UnlessAfter(&[b"#\\"]))];
const COFFEESCRIPT: &[Literal] =
    &[TRIPLE, lit(b"'''", b'\\', true, Plain), SPANNING, lit(b"'", b'\\', true, Plain)];
/// Haxe's strings span lines with either quote.
const HAXE: &[Literal] = &[SPANNING, lit(b"'", b'\\', true, Plain)];
/// Jsonnet's text blocks open and close at `|||`, and its verbatim strings `@"..."` and `@'...'`
/// double their quotes.
const JSONNET: &[Literal] = &[
    lit(b"|||", 0, true, Plain),
    lit(b"\"", b'"', true, After(b"@")),
    lit(b"'", b'\'', true, After(b"@")),
    SPANNING,
    lit(b"'", b'\\', true, Plain),
];
/// Meson's multiline strings have no escapes.
const MESON: &[Literal] = &[lit(b"'''", 0, true, Plain), lit(b"'", b'\\', false, Plain)];
/// Nushell's single-quoted and backquoted strings have no escapes.
const NUSHELL: &[Literal] = &[SPANNING, lit(b"'", 0, true, Plain), lit(b"`", 0, false, Plain)];
/// In Prolog, `0'c` is the code of the character `c`, so a quote after `0` opens no atom.
const PROLOG: &[Literal] =
    &[lit(b"\"", b'\\', false, Plain), lit(b"'", b'\\', false, UnlessAfter(&[b"0"]))];

/// The string literals of the language of `ext`, in the order in which they are tried. A `'` that
/// does not open one of them starts a character literal or, in Rust, a lifetime.
pub(super) fn literals(ext: &str) -> &'static [Literal] {
    match ext {
        "rs" => RUST,
        "go" => GO,
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "mts" | "cjs" | "cts" => JAVASCRIPT,
        // C has C's strings, as any language with C's comments and no arm of its own does, but its
        // arm comes early, so that the most common language does not pass every arm below.
        "c" => &[QUOTED],
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
        // The arms from here on are of languages that are rarer than those above, so that a lookup
        // for a common language finds its arm before them.
        "hxx" | "h++" | "c++" | "inl" | "ipp" | "tpp" | "cppm" | "ccm" | "mxx" | "ixx" | "cu"
        | "cuh" | "metal" => CPP,
        "pyi" | "pyw" | "pyx" | "pxd" | "pxi" | "scons" | "groovy" | "gvy" | "gy" | "gsh" => PYTHON,
        "csx" => CSHARP,
        "sc" => KOTLIN,
        "fs" | "fsi" | "fsx" => FSHARP,
        "pas" | "dpr" | "lpr" => PASCAL,
        "d" | "di" => D,
        "el" => EMACS_LISP,
        "scm" | "ss" | "sld" | "sls" | "rkt" | "lisp" | "lsp" | "asd" => LISP,
        "coffee" => COFFEESCRIPT,
        "graphql" | "gql" | "elm" => &[TRIPLE, QUOTED],
        "purs" | "nim" | "nims" | "nimble" => RAW_TRIPLE,
        "vhd" | "vhdl" => VHDL,
        "thrift" => SOLIDITY,
        "just" => TOML,
        "odin" => GO,
        "tcl" | "tk" | "gleam" => &[SPANNING],
        // Stylus and JSON5 have the same quotes as Solidity.
        "styl" | "json5" => SOLIDITY,
        "qml" => JAVASCRIPT,
        "hx" => HAXE,
        "jsonnet" | "libsonnet" => JSONNET,
        "meson.build" => MESON,
        "nu" => NUSHELL,
        "prisma" => &[QUOTED],
        "prolog" => PROLOG,
        "as" | "cfc" => SOLIDITY,
        _ => &[],
    }
}
