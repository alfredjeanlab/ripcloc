# ripcloc

ripcloc counts lines of code by language, and splits them into source and tests.
It is inspired by ripgrep.

## Installation

```
cargo install --git https://github.com/alfredjeanlab/ripcloc
```

## Usage

```
ripcloc                 # the current directory
ripcloc src lib/a.py    # any mix of files and directories
ripcloc --depth 2       # top-level directories broken down into their packages
ripcloc -o json
```

```
──────────────────────────────────────────────────────────────
Language                  files     blank   comment      code
──────────────────────────────────────────────────────────────
Rust (source)                 5       119       118      1191
  ripcloc                     5       119       118      1191
Rust (tests)                  9       205       204      1501
  ripcloc                     4       161       151      1133
  tests                       5        44        53       368
TOML                          8         8         1        62
Markdown                      2        17         0        52
Makefile                      1         4         0        11
──────────────────────────────────────────────────────────────
Source total                  5       119       118      1191
Test total                    9       205       204      1501
Other total                  11        29         1       125
──────────────────────────────────────────────────────────────
Total                        25       353       323      2817
──────────────────────────────────────────────────────────────
```

## Tests

Each project's language is detected from its manifest, such as `Cargo.toml`, `go.mod`, `package.json`, `pyproject.toml` or `Gemfile`, and its conventions decide which files are tests.
In Rust, inline `#[cfg(test)]` modules count as tests too.

A subdirectory with its own manifest is a project of its own.
Workspace members, a workspace root that is also a package, and nested projects are counted as packages.

## Skipped

Files ignored by git, hidden files, and build and dependency directories such as `target/` and `node_modules/`.

`NO_COLOR` disables color, and `COLOR` forces it.
