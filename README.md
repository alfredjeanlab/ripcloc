# ripcloc

ripcloc quickly counts lines of code by language.

It finds projects by manifest (e.g. `Cargo.toml` or `package.json`) and understands each language and framework's conventions.
It is generally faster than similar tools, often two or three times faster on large repositories (and more than 100x faster than `cloc`).

Here it is on its own repository:

```
────────────────────────────────────────────────────────────────────────
Language                  files     lines     blank   comment      code
────────────────────────────────────────────────────────────────────────
Rust (source)                 5      2245       184       289      1772
  ripcloc                     5      2245       184       289      1772
Rust (tests)                  7      2008       209       218      1581
  ripcloc                     2      1346       150       144      1052
  tests                       5       662        59        74       529
Python                        2        68         8         1        59
  bench                       2        68         8         1        59
Markdown                      2       142        37         0       105
TOML                          8        72         8         1        63
Makefile                      1        15         4         0        11
────────────────────────────────────────────────────────────────────────
Source total                  7      2313       192       290      1831
Test total                    7      2008       209       218      1581
Other total                  11       229        49         1       179
────────────────────────────────────────────────────────────────────────
Total                        25      4550       450       509      3591
────────────────────────────────────────────────────────────────────────
```

## Source and tests

ripcloc splits each language into source and tests, using the conventions of its ecosystem:

  `_test.go` files in Go, `tests/` and `benches/` in Rust,`__tests__/` and `*.test.ts` in JavaScript,
  `test_*.py` and `conftest.py` in Python, `spec/` in Ruby, and so on.

Rust's inline `#[cfg(test)]` modules are tests too.
Their lines are counted as tests, even inside a source file.

`--no-attribution` lists one row per language, as other line counters do.

## Monorepos

A directory with its own `Cargo.toml`, `package.json`, `go.mod`, `pyproject.toml` or `Gemfile` is a project.
A Python service inside a Rust workspace still has Python's tests.

Each language is broken down by top-level directory and by workspace, package, or module for a variety of
ecosystems and package managers (Cargo, Go modules, npm, yarn, pnpm, etc)
so you can see where the code is as well as how much there is.

Each workspace nested below the top level adds a level, and `--depth` sets how many levels are shown, which is one by default.
So `--depth 2` also lists the packages of a workspace inside a top-level directory, and `--depth 0` turns the breakdown off.

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
ripcloc -o json             # the same counts as JSON
ripcloc --depth 2           # also the packages of nested workspaces
ripcloc --no-attribution    # one row per language, as other counters show it
ripcloc --no-ignore         # count what .gitignore leaves out
```

ripcloc skips what git ignores, hidden files, symbolic links inside the directories it walks, and each ecosystem's build and dependency directories, such as `target/`, `node_modules/`, `vendor/` and `.venv/`.
Files over 1 GiB are skipped unless you pass `--large`, and `--no-large` skips files over 10 MB.
Markdown, YAML, TOML, Makefiles and other documentation and configuration are counted, but not split into source and tests.

`NO_COLOR` turns color off, and `COLOR` forces it on.

## Benchmarks

Counting whole repositories, with each tool's default settings:

| Repository | ripcloc | [scc](https://github.com/boyter/scc) | [tokei](https://github.com/XAMPPRocky/tokei) | [cloc](https://github.com/AlDanial/cloc) |
| ---------- | ------: | --: | ----: | ---: |
| linux | **234ms** | 650ms (2.8x) | 794ms (3.4x) | 96.7s (413x) |
| rust | **106ms** | 180ms (1.7x) | 300ms (2.8x) | 15.9s (150x) |
| react | **22ms** | 48ms (2.2x) | 46ms (2.1x) | 3.0s (135x) |
| rails | **17ms** | 31ms (1.8x) | 35ms (2.1x) | 1.95s (115x) |

The tools disagree about which files are code, so these counted different numbers of files and lines.
Here each tool counts the same files, of the languages they all count alike, with ripcloc's attribution and scc's complexity turned off:

| Files counted | Files | ripcloc | scc | tokei |
| ------------- | ----: | ------: | --: | ----: |
| C and headers in linux | 65,049 | **177ms** | 411ms (2.3x) | 646ms (3.6x) |
| Rust in rust | 39,811 | **68ms** | 126ms (1.9x) | 174ms (2.6x) |
| JavaScript and TypeScript in react | 4,449 | **13ms** | 26ms (2.1x) | 28ms (2.2x) |

Their code line counts agree to within 1.2%.
A single small file takes ripcloc 1.3ms, tokei 4.3ms and scc 7.7ms.

These are the means of at least ten runs through [hyperfine](https://github.com/sharkdp/hyperfine), on a warm cache, using 15 cores of an AMD EPYC machine running Linux.

## JSON

`-o json` prints the same rows as the text, as JSON:

```
{
  "schema": 1,
  "attribution": true,
  "languages": [
    { "language": "Rust", "kind": "source", "files": 5, "lines": 2245, "blank": 184, "comment": 289, "code": 1772,
      "packages": [{ "name": "ripcloc", "files": 5, "lines": 2245, ... }] },
    ...
  ],
  "totals": { "source": {...}, "test": {...}, "other": {...}, "total": {...} }
}
```

## Compatibility

ripcloc respects semantic versioning for: CLI arguments, JSON output, exit codes and environment variables.

A major release may remove or rename a flag or a JSON field, changes what one means, or changes a flag's default.  
A minor release may count new languages or find comments and tests more accurately.  

The text output may change in any release. Scripts should use `-o json`.
