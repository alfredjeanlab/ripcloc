// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! The languages that are counted, and the keys that select them from a file's name or content.

use super::{find, line_end, lines, trim, trim_end, trim_start};

/// A language or format that is counted, with the extensions and file names that select its files.
#[derive(Debug)]
pub struct Counted {
    pub name: &'static str,
    /// Whether its files are split into source and tests, rather than counted as other.
    pub split: bool,
    /// The extensions, in lower case, each of which is the key of the files that have it.
    pub extensions: &'static [&'static str],
    /// The extensions that it shares with another language, whose files are its own when their
    /// content says so.
    pub shared: &'static [&'static str],
    /// The file names that select it, which match in their case.
    pub names: &'static [&'static str],
}

/// Defines [`LANGUAGES`] and the lookups that read it.
///
/// A language's key is its first extension, unless its entry names a `key` before its extensions,
/// which it does only when the key is not one of them, as `make` and `dockerfile` are not. Each
/// lookup is a `match` that the compiler builds from the table. Its arms are tried in the order of
/// the table, so the most common languages come first.
macro_rules! languages {
    ($($name:literal $split:ident $(key $key:literal)? [$($ext:literal),*]
        $(shared [$($shared:literal),*])? $(names [$($file:literal),*])?;)*) => {
        /// Every language and format that is counted, in the order in which keys are looked up.
        pub const LANGUAGES: &[Counted] = &[$(Counted {
            name: $name,
            split: languages!(@split $split),
            extensions: &[$($ext),*],
            shared: &[$($($shared),*)?],
            names: &[$($($file),*)?],
        }),*];

        /// The name of the language of files with the key `key`, which is the key itself when it
        /// is not counted.
        pub fn language_name(key: &str) -> &str {
            match key {
                $($(| $key)? $(| $ext)* => $name,)*
                other => other,
            }
        }

        /// Whether files with the key `key` are split into source and tests. Documentation, data,
        /// configuration and build files are not.
        pub fn is_split(key: &str) -> bool {
            match key {
                $($(| $key)? $(| $ext)* => languages!(@split $split),)*
                _ => true,
            }
        }

        /// Whether files with the key `key` are counted.
        pub fn is_counted(key: &str) -> bool {
            match key {
                $($(| $key)? $(| $ext)* => true,)*
                _ => false,
            }
        }

        languages!(@names $(($($key)? [$($ext),*]) [$($($file),*)?])*);
    };
    // The names are matched in a rule of their own, because the arm of a name needs the key of its
    // language, and a macro cannot repeat the optional key inside the repetition of the names.
    (@names $($keys:tt [$($file:literal),*])*) => {
        /// The key of the files named `name`, if the name selects a language.
        fn name_key(name: &str) -> Option<&'static str> {
            match name {
                $($($file => Some(languages!(@key $keys)),)*)*
                _ => None,
            }
        }
    };
    (@key ($key:literal [$($ext:literal),*])) => { $key };
    (@key ([$first:literal $(, $ext:literal)*])) => { $first };
    (@split split) => { true };
    (@split other) => { false };
}

languages! {
    "Rust" split ["rs"];
    "Go" split ["go"];
    "Python" split ["py", "pyi", "pyw"];
    "Ruby" split ["rb", "rake", "gemspec"] names ["Rakefile", "rakefile", "Gemfile"];
    "ERB" split ["erb"];
    "JavaScript" split ["js", "jsx", "mjs", "cjs"];
    "TypeScript" split ["ts", "tsx", "mts", "cts"];
    "Java" split ["java"];
    "Kotlin" split ["kt", "kts"];
    // A `.sc` file is a Scala worksheet or script rather than SuperCollider, since Scala is the
    // more common of the two.
    "Scala" split ["scala", "sc"];
    "Swift" split ["swift"];
    "C" split ["c"];
    "C++" split [
        "cpp", "hpp", "cc", "cxx", "hh", "hxx", "h++", "c++", "inl", "ipp", "tpp", "cppm", "ccm",
        "mxx", "ixx"
    ];
    "C/C++ Header" split ["h"];
    "C#" split ["cs", "csx"];
    "Objective-C" split ["m"];
    "Objective-C++" split ["mm"];
    // Shell startup files are hidden, so they are counted only when a path names them, but
    // dotfiles repositories often keep them under names without the dot. A file named `profile`
    // without the dot is as often the configuration of another program, so it is not counted.
    "Shell" split ["sh", "bash", "zsh", "fish", "bats", "ksh", "csh", "tcsh"] names [
        ".bashrc", ".bash_profile", ".zshrc", ".zprofile", ".zshenv", ".profile", "bashrc",
        "bash_profile", "zshrc", "zprofile", "zshenv"
    ];
    "PowerShell" split ["ps1", "psm1", "psd1"];
    "Batch" split ["bat", "cmd"];
    "PHP" split ["php"];
    "Lua" split ["lua", "luau"];
    "SQL" split ["sql"];
    "R" split ["r"];
    "Perl" split ["pl", "pm"];
    "Vue" split ["vue"];
    "Svelte" split ["svelte"];
    "TOML" other ["toml"];
    "YAML" other ["yaml", "yml"];
    "JSON" other ["json"];
    "Markdown" other ["md", "markdown"];
    "HTML" split ["html", "htm"];
    // MSBuild projects and their imports, XAML, resources, NuGet specifications and PowerShell's
    // type and format files are XML. A `.vcxproj.filters` file has the key `vcxproj`.
    "XML" split [
        "xml", "csproj", "vbproj", "fsproj", "props", "targets", "xaml", "resx", "nuspec",
        "vcxproj", "vcxproj.filters", "ps1xml"
    ];
    "CSS" split ["css"];
    "SCSS" split ["scss"];
    "Sass" split ["sass"];
    "LESS" split ["less"];
    "INI" split ["ini"];
    "Protocol Buffers" split ["proto"];
    "Makefile" other key "make" ["mk", "mak"] names ["Makefile", "makefile", "GNUmakefile"];
    "Dockerfile" other key "dockerfile" [] names ["Dockerfile"];
    "CMake" split ["cmake"] names ["CMakeLists.txt"];
    "Device Tree" split ["dts", "dtsi"];
    // GNU and Intel style assembly are one language, as in cloc, because a count of assembly says
    // more than a count for each assembler.
    "Assembly" split ["s", "asm"];
    "reStructuredText" other ["rst"];
    "M4" other ["m4"];
    "Autoconf" other key "autoconf" [] names ["configure.ac", "configure.in"];
    "Automake" other key "automake" [] names ["Makefile.am"];
    "AWK" split ["awk"];
    "Lex" split ["l", "lex"];
    "Yacc" split ["y", "yy"];
    "Linker Script" other ["ld", "lds"];
    "TeX" other ["tex", "sty"];
    "Zig" split ["zig", "zon"];
    "Haskell" split ["hs"];
    "Elixir" split ["ex", "exs"];
    "OCaml" split ["ml", "mli"];
    "Dart" split ["dart"];
    "Julia" split ["jl"];
    "Clojure" split ["clj", "cljs", "cljc"];
    "Erlang" split ["erl", "hrl"];
    "Nix" split ["nix"];
    "Terraform" split ["tf"];
    "Terraform" other ["tfvars", "hcl"];
    "Lean" split ["lean"];
    "MDX" other ["mdx"];
    "Solidity" split ["sol"];
    "Gradle" other ["gradle"];
    "Visual Basic" split ["vb"];
    "AsciiDoc" other ["adoc", "asciidoc"];
    // `BUILD.bazel` and `MODULE.bazel` have the extension `bazel`.
    "Bazel" other ["bazel", "bzl", "bzlmod", "star"] names ["BUILD", "WORKSPACE"];
    // A `.pp` file is Puppet rather than Free Pascal, since Free Pascal units
    // are mostly named `.pas`.
    "Puppet" split ["pp"];
    "Jinja" split ["j2", "jinja", "jinja2"];
    "Verilog" split ["v", "vh"];
    "SystemVerilog" split ["sv", "svh"];
    "Coq" split key "coq" [] shared ["v"];
    // A `.fs` file is F# rather than a GLSL fragment shader or Forth.
    "F#" split ["fs", "fsi", "fsx"];
    // Fixed-form sources, whose comments are marked in the first column, have the extensions
    // `.f`, `.for` and `.f77`.
    "Fortran" split ["f90", "f95", "f03", "f08", "f", "for", "f77"];
    "CUDA" split ["cu", "cuh"];
    "Groovy" split ["groovy", "gvy", "gy", "gsh"] names ["Jenkinsfile"];
    "GraphQL" split ["graphql", "gql"];
    "Cython" split ["pyx", "pxd", "pxi"];
    "Pascal" split ["pas", "dpr", "lpr"];
    // A `.d` file that a compiler writes to list the dependencies of a target is not D, and is
    // not counted.
    "D" split ["d", "di"];
    "Tcl" split ["tcl", "tk"];
    "Emacs Lisp" split ["el"];
    "Scheme" split ["scm", "ss", "sld", "sls"];
    "Racket" split ["rkt"];
    // A `.cl` file may be OpenCL as well as Common Lisp, so it is not counted.
    "Common Lisp" split ["lisp", "lsp", "asd"];
    "Ada" split ["adb", "ads", "ada"];
    "COBOL" split ["cbl", "cob", "cobol", "cpy"];
    "Handlebars" split ["hbs", "handlebars"];
    "Mustache" split ["mustache"];
    "Pug" split ["pug", "jade"];
    "CoffeeScript" split ["coffee"];
    "Elm" split ["elm"];
    "PureScript" split ["purs"];
    "VHDL" split ["vhd", "vhdl"];
    "GLSL" split ["glsl", "vert", "frag", "geom", "comp", "tesc", "tese"];
    "HLSL" split ["hlsl", "fx", "fxh"];
    "Metal" split ["metal"];
    "SCons" other key "scons" [] names ["SConstruct", "SConscript"];
    "Just" other ["just"] names ["justfile", "Justfile", ".justfile"];
    "Thrift" split ["thrift"];
    "Visual Studio Solution" other ["sln"];
    "Nim" split ["nim", "nims", "nimble"];
    "Crystal" split ["cr"];
    "Gleam" split ["gleam"];
    "Odin" split ["odin"];
    "Typst" other ["typ"];
    // JSONC and JSON5 are languages of their own rather than JSON, since their comments make
    // them count differently from strict JSON.
    "JSONC" other ["jsonc"];
    "JSON5" other ["json5"];
    "Stylus" split ["styl"];
    // The key has a dot, so that it is not the extension of any file. A `.meson` file is a
    // configuration of Meson's formatter or of RPM's macros, rather than a build file.
    "Meson" other key "meson.build" [] names ["meson.build", "meson_options.txt", "meson.options"];
    "Vim Script" split ["vim"] names [
        "vimrc", ".vimrc", "_vimrc", "gvimrc", ".gvimrc", "_gvimrc"
    ];
    "Prisma" split ["prisma"];
    "Nushell" split ["nu"];
    "QML" split ["qml"];
    "Haxe" split ["hx"];
    "Jsonnet" other ["jsonnet", "libsonnet"];
    // A `.fs` file stays F#, and a `.f` file stays Fortran, so Forth is counted only with the
    // extensions that no other language has.
    "Forth" split ["fth", "4th", "forth"];
    // Pharo's Tonel files, such as `A.class.st` and `A.extension.st`, have the extension `st`.
    "Smalltalk" split ["st"];
    // A `.pl` file is Perl unless its content says that it is Prolog. A `.pro` file is as often a
    // Qt project file as Prolog, so it is not counted.
    "Prolog" split key "prolog" [] shared ["pl"];
    "Jupyter Notebook" split ["ipynb"];
    "ASP.NET" split ["aspx", "ascx", "asmx", "ashx", "asax", "master"];
    "Razor" split ["cshtml", "vbhtml", "razor"];
    "ASP" split ["asp"];
    "SVG" other ["svg"];
    // A `.as` file is ActionScript rather than AngelScript, since far more ActionScript is
    // written in files with that extension.
    "ActionScript" split ["as"];
    // ColdFusion's pages and its components are languages of their own, since a component is
    // mostly CFScript, and a component is counted as CFScript even when it is written in tags.
    "ColdFusion" split ["cfm", "cfml"];
    "ColdFusion CFScript" split ["cfc"];
    "Twig" split ["twig"];
}

/// The key that the tables in this module use for a file: the key of a name in [`LANGUAGES`],
/// such as `Makefile`, or else the lowercased extension.
pub fn file_key(path: &std::path::Path) -> String {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if let Some(key) = name_key(name) {
        return key.to_string();
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    // The filters of a Visual C++ project are XML, while other files whose names end in
    // `.filters` are not counted.
    if ext == "filters" && name.to_lowercase().ends_with(".vcxproj.filters") {
        return "vcxproj".to_string();
    }
    ext
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
        "swipl" => "prolog",
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
/// a Verilog file almost always has a `module` or a statement that ends in `;` near its start.
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

/// Whether a `.d` file whose content starts with `content` lists the dependencies of a target, as
/// `gcc -MD`, `rustc --emit=dep-info` and other compilers write them, rather than being D.
///
/// Such a file starts with a rule, such as `foo.o: foo.c foo.h \`, whose targets are paths. The
/// first line is taken for a rule when it has a `:` right after a target, followed by whitespace
/// or the end of the line, and the text before the `:` has a `.` or a `/`, as a path does. A D
/// label such as `private:` has neither, and a line with a byte that D's code has and a rule does
/// not, such as `(` in `version (Windows):` or `;` in `import std.stdio : writeln;`, is not a
/// rule, nor is a line that starts with a comment, `import` or `module`. Only the first
/// [`D_PREFIX`] bytes are read.
pub fn is_make_dependencies(content: &[u8]) -> bool {
    let start = content.get(..D_PREFIX).unwrap_or(content);
    let line = line_end(trim_start(start));
    let code = [b"//".as_slice(), b"/*", b"/+", b"import ", b"module "];
    if code.iter().any(|prefix| line.starts_with(prefix))
        || line.iter().any(|b| b"(){};=\"'@#".contains(b))
    {
        return false;
    }
    let mut targets = false;
    for (at, &byte) in line.iter().enumerate() {
        match byte {
            b':' if line.get(at + 1).is_none_or(u8::is_ascii_whitespace)
                && at
                    .checked_sub(1)
                    .and_then(|p| line.get(p))
                    .is_some_and(|b| !b.is_ascii_whitespace()) =>
            {
                return targets;
            }
            b'.' | b'/' => targets = true,
            _ => {}
        }
    }
    false
}

/// The most bytes at the start of a `.d` file that are read to tell a list of dependencies from D.
pub(super) const D_PREFIX: usize = 1024;

/// The key of a `.pl` file whose content starts with `content`, which is `prolog` for Prolog and
/// `pl` for Perl.
///
/// A `#!` line that names `perl` or `swipl` decides at once. Otherwise each line is scored for
/// each language, and Prolog must score higher to win, so that a tie, which includes a file with no
/// signal at all, stays Perl, as every `.pl` file was before Prolog was counted. A directive such
/// as `:- module(` is a strong sign of Prolog, and `use strict` and `my $` are strong signs of
/// Perl. A `%` comment, a line that ends in `:-` or `.`, and a clause with `:-` in it are weaker
/// signs of Prolog, and a `#` comment, a line that ends in `;`, `{` or `}`, and a sigil such as
/// `$x` are weaker signs of Perl.
///
/// The lines of a `/* */` comment and of Perl's POD are skipped, and scanning stops at Perl's
/// `__END__` or `__DATA__`. It also stops at the end of the first line after which the scores
/// differ by [`PL_MARGIN`], and it never goes past [`PL_PREFIX`].
pub fn pl_key(content: &[u8]) -> &'static str {
    let start = content.get(..PL_PREFIX).unwrap_or(content);
    if let Some(shebang) = start.strip_prefix(b"#!") {
        let shebang = line_end(shebang);
        if find(shebang, b"swipl").is_some() {
            return "prolog";
        }
        if find(shebang, b"perl").is_some() {
            return "pl";
        }
    }
    let (mut prolog, mut perl) = (0u32, 0u32);
    let (mut in_comment, mut in_pod) = (false, false);
    for line in lines(start) {
        let mut line = trim(line);
        if in_comment {
            let Some(end) = find(line, b"*/") else { continue };
            line = trim(line.get(end + 2..).unwrap_or_default());
            in_comment = false;
        }
        if in_pod {
            in_pod = !line.starts_with(b"=cut");
            continue;
        }
        if line.starts_with(b"__END__") || line.starts_with(b"__DATA__") {
            perl += 3;
            break;
        }
        let (prolog_weight, perl_weight) = pl_line(line, &mut in_comment, &mut in_pod);
        (prolog, perl) = (prolog + prolog_weight, perl + perl_weight);
        if prolog.abs_diff(perl) >= PL_MARGIN {
            break;
        }
    }
    if prolog > perl { "prolog" } else { "pl" }
}

/// The scores for Prolog and for Perl of `line`, a line of a `.pl` file without its surrounding
/// whitespace. `in_comment` and `in_pod` are set when a `/*` comment or POD opens on it and does
/// not close.
fn pl_line(line: &[u8], in_comment: &mut bool, in_pod: &mut bool) -> (u32, u32) {
    let starts = |prefix: &[u8]| line.starts_with(prefix);
    match line {
        [] => return (0, 0),
        [b'/', b'*', rest @ ..] => {
            *in_comment = find(rest, b"*/").is_none();
            return (1, 0);
        }
        // A `%` that a word or a sigil follows is a Perl hash, as in `%ENV` or `%$h`.
        [b'%', b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'$' | b'{' | b'(', ..] => return (0, 1),
        [b'%', ..] => return (1, 0),
        [b'#', ..] => return (0, 1),
        // POD starts with a command such as `=head1` or `=pod`.
        [b'=', b'a'..=b'z', ..] => {
            *in_pod = !starts(b"=cut");
            return (0, 3);
        }
        _ => {}
    }
    if starts(b":- module(") || starts(b":- use_module(") {
        return (4, 0);
    }
    if starts(b":-") {
        return (2, 0);
    }
    if starts(b"use strict") || starts(b"use warnings") {
        return (0, 4);
    }
    let mut perl = 0;
    if starts(b"my ") || starts(b"our ") || starts(b"sub ") || starts(b"package ") {
        perl += 2;
    }
    if starts(b"use ") || starts(b"require ") {
        perl += 1;
    }
    // A sigil before a name, which a Prolog variable does not have. A `$` inside a quoted atom,
    // as in `'$skip'`, follows the quote.
    let sigil = memchr::memchr_iter(b'$', line).any(|at| {
        let after =
            line.get(at + 1).is_some_and(|&b| b.is_ascii_alphabetic() || b"_{".contains(&b));
        let before = at.checked_sub(1).and_then(|p| line.get(p));
        after && before != Some(&b'\'')
    });
    perl += u32::from(sigil);
    // The code of the line, without a trailing `%` or `#` comment after a space.
    let code = [b" % ".as_slice(), b" # "]
        .iter()
        .filter_map(|comment| find(line, comment))
        .min()
        .map_or(line, |at| trim_end(line.get(..at).unwrap_or_default()));
    let mut prolog = u32::from(find(code, b":-").is_some());
    match code.last() {
        Some(b'.') => prolog += 1,
        Some(b';' | b'{' | b'}') => perl += 1,
        _ => {}
    }
    if code.ends_with(b":-") {
        prolog += 1;
    }
    (prolog, perl)
}

/// How much one language must outscore the other at the end of a line for [`pl_key`] to stop.
const PL_MARGIN: u32 = 8;

/// The most bytes at the start of a `.pl` file that are read to tell Prolog from Perl.
pub(super) const PL_PREFIX: usize = 4096;

/// The most bytes at the start of a `.v` file that are read to tell Coq from Verilog.
///
/// A license header and the code after it fit, since a header of a few kilobytes is skipped with
/// one search for its end. The bound is that of the binary check in `main.rs`, which already reads
/// these bytes, so the worst case costs about as much as that check.
pub(super) const V_PREFIX: usize = 8000;
