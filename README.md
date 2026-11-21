# ripcloc

ripcloc counts lines of code very fast.

It is typically nearly three times faster than similar tools, uses less memory, and is more than 150x faster than `cloc`.

By default, line counts are grouped by project (via `Cargo.toml`, `package.json`, `pyproject.toml`, etc)
and attributed as either "source" or "tests" in a language and framework aware way.

Here it is on [hyper](https://github.com/hyperium/hyper) at `21da6a5`:

```
────────────────────────────────────────────────────────────────────────
Language                  files     lines     blank   comment      code
────────────────────────────────────────────────────────────────────────
Rust (source)                80     20233      2175      4277     13781
  examples                   16      1870       297       243      1330
  src                        64     18363      1878      4034     12451
Rust (tests)                 19     15910      2092       624     13194
  benches                     7      1135       156        46       933
  src                         0      4813       571       216      4026
  tests                      12      9962      1365       362      8235
C                             2       743       156        49       538
C/C++ Header                  1       544        97       300       147
Shell                         1        62         9        13        40
HTML                          1        10         0         0        10
Markdown                     22      5280      2070         0      3210
TOML                          2       366        54        13       299
Makefile                      1        25         7         3        15
SVG                           1         4         0         1         3
────────────────────────────────────────────────────────────────────────
Total source                 85     21592      2437      4639     14516
Total tests                  19     15910      2092       624     13194
Total other                  26      5675      2131        17      3527
────────────────────────────────────────────────────────────────────────
Total                       130     43177      6660      5280     31237
────────────────────────────────────────────────────────────────────────
```

## Source and tests

ripcloc splits each language into source and tests, using the conventions of its ecosystem:

  `_test.go` files in Go, `tests/` and `benches/` in Rust, `__tests__/` and `*.test.ts` in JavaScript,
  `test_*.py` and `conftest.py` in Python, `spec/` in Ruby, `AppTests/` targets in Xcode,
  `src/test/` and `src/androidTest/` in Gradle, `integration_test/` in Flutter, and so on.

Rust's inline `#[cfg(test)]` modules are tests too.
Their lines are counted as tests, even inside a source file.

## Monorepos

A directory with its own manifest, such as `Cargo.toml`, `package.json` or `go.mod`, is a project.
A Python service inside a Rust workspace still has Python's tests.

Each language is broken down by top-level directory and by workspace, package, or module,
so you can see where the code is as well as how much there is.

## Installation

```
brew install alfredjeanlab/tap/ripcloc
cargo install ripcloc
```

Binaries for Linux and macOS are on the [releases page](https://github.com/alfredjeanlab/ripcloc/releases).

## Usage

```
ripcloc                     # the current directory
ripcloc src lib/a.py        # any mix of files and directories
ripcloc -c                  # one line per language
ripcloc -o json             # the same counts as JSON
ripcloc --depth 2           # also the packages of nested workspaces
ripcloc --no-ignore         # count what .gitignore leaves out
ripcloc --deps              # also count vendor/, node_modules/ and virtual environments
ripcloc --languages         # the languages and file names that it counts
```

ripcloc skips binaries, hidden files, ignored files, build output and dependencies, such as `target/` in Rust and `.venv/` in Python.

## Benchmarks

Counting whole repositories and a single file, with each tool's default settings:

| Repository | ripcloc | [scc](https://github.com/boyter/scc) | [tokei](https://github.com/XAMPPRocky/tokei) | [cloc](https://github.com/AlDanial/cloc) |
| ---------- | ------: | --: | ----: | ---: |
| linux | **230ms** | 676ms (2.9x) | 942ms (4.1x) | 123s (537x) |
| rust | **107ms** | 189ms (1.8x) | 360ms (3.4x) | 20.5s (192x) |
| react | **22ms** | 51ms (2.3x) | 54ms (2.5x) | 3.6s (165x) |
| rails | **17ms** | 33ms (2.0x) | 48ms (2.9x) | 2.2s (133x) |
| single file | **2.9ms** | 19ms (6.7x) | 7.5ms (2.6x) | 118ms (41x) |

The tools disagree about which files are code, so these counted different numbers of files and lines.
Here each tool counts the same files, of the languages they all count alike, with ripcloc's attribution and scc's complexity turned off:

| Files counted | Files | ripcloc | scc | tokei |
| ------------- | ----: | ------: | --: | ----: |
| C and headers in linux | 65,049 | **178ms** | 434ms (2.4x) | 769ms (4.3x) |
| Rust in rust | 39,802 | **77ms** | 135ms (1.7x) | 235ms (3.0x) |
| JavaScript and TypeScript in react | 4,449 | **14ms** | 29ms (2.1x) | 39ms (2.8x) |

| Peak memory | ripcloc | [scc](https://github.com/boyter/scc) | [tokei](https://github.com/XAMPPRocky/tokei) |
| ----------- | ------: | --: | ----: |
| single file | **4.2 MiB** | 16 MiB | 5.2 MiB |
| ripgrep, 154 files | **6.2 MiB** | 26 MiB | 8.1 MiB |
| linux, 90,000 files | **63 MiB** | 363 MiB | 272 MiB |

Times are the means of ten runs on a warm cache (one for cloc), and memory the median of three, pinned to 15 of 16 dedicated AMD EPYC vCPUs running Ubuntu 26.04.
The raw results and every other benchmark are in [bench/runs](bench/runs/2026-11-20-ubuntu2604-fuzz-aeb7c76).

## Compatibility

ripcloc follows semantic versioning for its CLI arguments, JSON output and exit codes.

A major release may remove or rename a flag or a JSON field, change what one means, or change a flag's default.
A minor release may count new languages or find comments and tests more accurately.

The text output may change in any release.
Scripts should use `-o json`, which is stable.
