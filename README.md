# ripcloc

ripcloc quickly counts lines of code by language.

It is typically nearly three times faster than similar tools, uses less memory, and is more than 100x faster than `cloc`.

By default, line counts are grouped by project (via  `Cargo.toml`, `package.json`, `pyproject.toml`, etc)
and attributed as either "source" or "tests" in a language and framework aware way.

Here it is on its own repository:

```
────────────────────────────────────────────────────────────────────────
Language                  files     lines     blank   comment      code
────────────────────────────────────────────────────────────────────────
Rust (source)                 5      2146       149       285      1712
  ripcloc                     5      2146       149       285      1712
Rust (tests)                  7      2807       305       215      2287
  ripcloc                     2      2071       239       133      1699
  tests                       5       736        66        82       588
Python                        2        88        10         1        77
  bench                       2        88        10         1        77
Markdown                      3       399        96         0       303
TOML                          8        77         8         1        68
Makefile                      1        15         4         0        11
────────────────────────────────────────────────────────────────────────
Source total                  7      2234       159       286      1789
Test total                    7      2807       305       215      2287
Other total                  12       491       108         1       382
────────────────────────────────────────────────────────────────────────
Total                        26      5532       572       502      4458
────────────────────────────────────────────────────────────────────────
```

## Source and tests

ripcloc splits each language into source and tests, using the conventions of its ecosystem:

  `_test.go` files in Go, `tests/` and `benches/` in Rust, `__tests__/` and `*.test.ts` in JavaScript,
  `test_*.py` and `conftest.py` in Python, `spec/` in Ruby, `AppTests/` targets in Xcode,
  `src/test/` and `src/androidTest/` in Gradle, `integration_test/` in Flutter, and so on.

Rust's inline `#[cfg(test)]` modules are tests too.
Their lines are counted as tests, even inside a source file.

`-c`/`--compact` lists one row per language, as other line counters do.

## Monorepos

A directory with its own `Cargo.toml`, `package.json`, `go.mod`, `pyproject.toml`, `Gemfile`, `pubspec.yaml`,
`Package.swift`, Xcode project, Gradle build or `pom.xml` is a project.
A Python service inside a Rust workspace still has Python's tests, and the `ios/` and `android/` apps of a
React Native project have Xcode's and Gradle's.

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
ripcloc -c                  # one line per language
ripcloc -o json             # the same counts as JSON
ripcloc --depth 2           # also the packages of nested workspaces
ripcloc --compact           # one row per language, as other counters show it
ripcloc --no-ignore         # count what .gitignore leaves out
```

ripcloc skips what git ignores, binary files, hidden files, symbolic links inside the directories it walks, and build and dependency directories. A directory that holds a `pyvenv.cfg` or `CACHEDIR.TAG` is skipped wherever it is, and a common name such as `target/` in Rust or `build/` in Python is skipped only directly inside the project that has the manifest, so a source package named `build` is still counted.
Files over 1 GiB are skipped unless you pass `--large`, and `--no-large` skips files over 10 MB.
Markdown, JSON, YAML, TOML, Makefiles and other documentation, data and configuration are counted, but not split into source and tests.

## Benchmarks

Counting whole repositories, with each tool's default settings:

| Repository | ripcloc | [scc](https://github.com/boyter/scc) | [tokei](https://github.com/XAMPPRocky/tokei) | [cloc](https://github.com/AlDanial/cloc) |
| ---------- | ------: | --: | ----: | ---: |
| linux | **230ms** | 676ms (2.9x) | 942ms (4.1x) | 123s (537x) |
| rust | **107ms** | 189ms (1.8x) | 360ms (3.4x) | 20.5s (192x) |
| react | **22ms** | 51ms (2.3x) | 54ms (2.5x) | 3.6s (165x) |
| rails | **17ms** | 33ms (2.0x) | 48ms (2.9x) | 2.2s (133x) |

The tools disagree about which files are code, so these counted different numbers of files and lines.
Here each tool counts the same files, of the languages they all count alike, with ripcloc's attribution and scc's complexity turned off:

| Files counted | Files | ripcloc | scc | tokei |
| ------------- | ----: | ------: | --: | ----: |
| C and headers in linux | 65,049 | **178ms** | 434ms (2.4x) | 769ms (4.3x) |
| Rust in rust | 39,802 | **77ms** | 135ms (1.7x) | 235ms (3.0x) |
| JavaScript and TypeScript in react | 4,449 | **14ms** | 29ms (2.1x) | 39ms (2.8x) |

A single small file takes ripcloc 2.9ms, tokei 7.5ms and scc 19ms.
ripcloc used the least memory in every benchmark, 56 to 110 MiB on linux where scc used 355 to 378 MiB and tokei 265 to 273 MiB.

These are the means of ten runs on a warm cache (one for cloc), pinned to 15 of 16 dedicated AMD EPYC vCPUs running Ubuntu 26.04.
The raw results and every other benchmark are in [bench/runs](bench/runs/2026-11-20-ubuntu2604-fuzz-aeb7c76).

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

ripcloc follows semantic versioning for its CLI arguments, JSON output and exit codes.

A major release may remove or rename a flag or a JSON field, change what one means, or change a flag's default.
A minor release may count new languages or find comments and tests more accurately.

The text output may change in any release. Scripts should use `-o json`.
