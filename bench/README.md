# Benchmarks

These scripts time ripcloc against [tokei](https://github.com/XAMPPRocky/tokei), [scc](https://github.com/boyter/scc)
and [cloc](https://github.com/AlDanial/cloc), in the manner of ripgrep's benchsuite.

## Method

Each command runs through [hyperfine](https://github.com/sharkdp/hyperfine) with three warmup runs and at least ten
timed ones, so every file is in the page cache.  hyperfine runs each command without a shell, and is itself pinned
to 15 of the machine's 16 cores, so that it times only the tool and every tool gets the same cores.

Each tool runs with its default settings, since that is what its users see. The tools disagree about which files are code,
since each knows a different set of languages and skips different directories. So beside each time is the number of files
and code lines that tool counted, and a tool that is fast because it skipped work shows it.

For a comparison of equal work, `bench/fetch` also makes copies of three corpora with only the files of languages that
every tool counts alike, which are the C files and headers of linux, the Rust files of rust, and the JavaScript and
TypeScript files of react.
With `SAME_WORK=1`, ripcloc runs with `--no-attribution --large` and scc with `--no-complexity`, so every tool does
the same work.
In these the tools count the same files, to within six, and their code lines agree to within 1.2%.

hyperfine times one command after another, so each benchmark was also run with the tools in the reverse order, in case a
slow drift of the machine favored whichever ran first.
The results did not change.

## Running them

```
cargo build --release
bench/fetch ~/corpora                  # about 4.5 GB
bench/run ~/corpora results            # every benchmark
bench/run ~/corpora results linux      # or some of them
python3 bench/report.py results linux  # a Markdown table of the results
```

`TOOLS` picks the tools and their order, from `ripcloc`, `ripcloc-noattr` (ripcloc with `--no-attribution`), `tokei`,
`scc`, `cloc`, `loc` and `gocloc`.
`SAME_WORK=1` runs the tools with equal work, as above.
On Linux, `CPUS` sets the cores that hyperfine is pinned to, by default `1-15`.

`environment.txt` in each results directory records the machine, the version of every tool, and the commit of every
corpus.

## Results

These were measured on 19 November 2026, on a 16-core AMD EPYC-Milan virtual machine with 61 GB of memory, running Linux
7.0 with hyperfine 1.19.0, tokei 15.0.0 and scc 4.1.0.
cloc 2.06 took 97 seconds on linux, 16 seconds on rust and 3 seconds on react in an earlier run, and is left out of
these, since every other tool is so much faster.
The raw results are in [results/2026-11-19](results/2026-11-19).

### Default settings

#### One large C file, sqlite3.c

| Tool | Files | Code lines | Time |
| ---- | ----: | ---------: | ---: |
| ripcloc `--no-attribution` | 1 | 165,971 | **15.3ms ± 0.5ms** (1.00x) |
| ripcloc | 1 | 165,971 | 15.5ms ± 0.4ms (1.01x) |
| [scc](https://github.com/boyter/scc) | 1 | 165,971 | 39.8ms ± 0.6ms (2.60x) |
| [tokei](https://github.com/XAMPPRocky/tokei) | 1 | 171,935 | 78.1ms ± 1.7ms (5.09x) |

#### One large Rust file, avx512f.rs

| Tool | Files | Code lines | Time |
| ---- | ----: | ---------: | ---: |
| ripcloc `--no-attribution` | 1 | 48,200 | **4.0ms ± 0.3ms** (1.00x) |
| ripcloc | 1 | 48,200 | 8.0ms ± 0.3ms (2.02x) |
| [scc](https://github.com/boyter/scc) | 1 | 48,200 | 18.0ms ± 0.6ms (4.56x) |
| [tokei](https://github.com/XAMPPRocky/tokei) | 1 | 48,200 | 18.2ms ± 0.6ms (4.60x) |

#### One large TypeScript file, checker.ts

| Tool | Files | Code lines | Time |
| ---- | ----: | ---------: | ---: |
| ripcloc `--no-attribution` | 1 | 42,977 | **4.9ms ± 0.2ms** (1.00x) |
| ripcloc | 1 | 42,977 | 5.2ms ± 0.3ms (1.05x) |
| [tokei](https://github.com/XAMPPRocky/tokei) | 1 | 42,977 | 14.9ms ± 0.5ms (3.04x) |
| [scc](https://github.com/boyter/scc) | 1 | 42,977 | 21.3ms ± 0.6ms (4.33x) |

#### One small Rust file

| Tool | Files | Code lines | Time |
| ---- | ----: | ---------: | ---: |
| ripcloc `--no-attribution` | 1 | 335 | **1.1ms ± 0.1ms** (1.00x) |
| ripcloc | 1 | 335 | 1.3ms ± 0.1ms (1.25x) |
| [tokei](https://github.com/XAMPPRocky/tokei) | 1 | 335 | 4.3ms ± 0.5ms (4.02x) |
| [scc](https://github.com/boyter/scc) | 1 | 335 | 7.7ms ± 0.5ms (7.26x) |

#### ripcloc

| Tool | Files | Code lines | Time |
| ---- | ----: | ---------: | ---: |
| ripcloc `--no-attribution` | 23 | 2,743 | **3.3ms ± 0.4ms** (1.00x) |
| ripcloc | 23 | 2,743 | 3.3ms ± 0.4ms (1.02x) |
| [tokei](https://github.com/XAMPPRocky/tokei) | 23 | 2,869 | 4.7ms ± 0.5ms (1.44x) |
| [scc](https://github.com/boyter/scc) | 26 | 2,925 | 9.1ms ± 0.5ms (2.80x) |

#### cloc

| Tool | Files | Code lines | Time |
| ---- | ----: | ---------: | ---: |
| ripcloc | 675 | 26,267 | **6.4ms ± 0.6ms** (1.00x) |
| ripcloc `--no-attribution` | 675 | 26,267 | 6.6ms ± 0.6ms (1.02x) |
| [tokei](https://github.com/XAMPPRocky/tokei) | 885 | 26,302 | 9.7ms ± 0.8ms (1.51x) |
| [scc](https://github.com/boyter/scc) | 894 | 64,359 | 37.5ms ± 1.9ms (5.82x) |

#### GNU grep 3.12

| Tool | Files | Code lines | Time |
| ---- | ----: | ---------: | ---: |
| ripcloc `--no-attribution` | 1,225 | 166,734 | **6.6ms ± 0.6ms** (1.00x) |
| ripcloc | 1,225 | 166,734 | 6.8ms ± 0.6ms (1.03x) |
| [tokei](https://github.com/XAMPPRocky/tokei) | 1,382 | 230,508 | 15.6ms ± 0.9ms (2.36x) |
| [scc](https://github.com/boyter/scc) | 1,382 | 261,404 | 19.4ms ± 1.3ms (2.94x) |

#### ripgrep

| Tool | Files | Code lines | Time |
| ---- | ----: | ---------: | ---: |
| ripcloc `--no-attribution` | 149 | 45,031 | **5.3ms ± 0.6ms** (1.00x) |
| ripcloc | 149 | 45,031 | 5.8ms ± 0.5ms (1.10x) |
| [tokei](https://github.com/XAMPPRocky/tokei) | 154 | 43,315 | 8.7ms ± 0.6ms (1.63x) |
| [scc](https://github.com/boyter/scc) | 193 | 54,048 | 11.9ms ± 0.7ms (2.24x) |

#### react

| Tool | Files | Code lines | Time |
| ---- | ----: | ---------: | ---: |
| ripcloc `--no-attribution` | 6,962 | 830,286 | **21.6ms ± 1.4ms** (1.00x) |
| ripcloc | 6,962 | 830,286 | 22.3ms ± 1.4ms (1.03x) |
| [tokei](https://github.com/XAMPPRocky/tokei) | 6,988 | 778,235 | 46.2ms ± 1.1ms (2.14x) |
| [scc](https://github.com/boyter/scc) | 7,058 | 825,668 | 48.0ms ± 1.5ms (2.22x) |

#### rails

| Tool | Files | Code lines | Time |
| ---- | ----: | ---------: | ---: |
| ripcloc `--no-attribution` | 4,516 | 517,161 | **16.1ms ± 0.6ms** (1.00x) |
| ripcloc | 4,516 | 517,161 | 17.0ms ± 0.6ms (1.05x) |
| [scc](https://github.com/boyter/scc) | 4,745 | 522,842 | 30.6ms ± 1.6ms (1.90x) |
| [tokei](https://github.com/XAMPPRocky/tokei) | 4,566 | 463,934 | 35.3ms ± 1.5ms (2.19x) |

#### rust

| Tool | Files | Code lines | Time |
| ---- | ----: | ---------: | ---: |
| ripcloc `--no-attribution` | 41,776 | 4,067,691 | **89.9ms ± 2.3ms** (1.00x) |
| ripcloc | 41,776 | 4,067,691 | 106.3ms ± 2.4ms (1.18x) |
| [scc](https://github.com/boyter/scc) | 44,331 | 4,216,656 | 180.0ms ± 3.2ms (2.00x) |
| [tokei](https://github.com/XAMPPRocky/tokei) | 43,196 | 4,036,065 | 300.3ms ± 18.3ms (3.34x) |

#### linux

| Tool | Files | Code lines | Time |
| ---- | ----: | ---------: | ---: |
| ripcloc `--no-attribution` | 89,872 | 33,293,243 | **225.9ms ± 7.9ms** (1.00x) |
| ripcloc | 89,872 | 33,293,243 | 234.2ms ± 9.7ms (1.04x) |
| [scc](https://github.com/boyter/scc) | 91,228 | 33,464,408 | 650.0ms ± 8.3ms (2.88x) |
| [tokei](https://github.com/XAMPPRocky/tokei) | 91,270 | 33,396,065 | 794.2ms ± 9.4ms (3.52x) |

### Equal work

#### C and headers in linux

| Tool | Files | Code lines | Time |
| ---- | ----: | ---------: | ---: |
| ripcloc | 65,049 | 28,851,688 | **177.3ms ± 7.2ms** (1.00x) |
| [scc](https://github.com/boyter/scc) | 65,049 | 28,851,109 | 410.9ms ± 5.0ms (2.32x) |
| [tokei](https://github.com/XAMPPRocky/tokei) | 65,049 | 28,856,188 | 646.0ms ± 7.2ms (3.64x) |

#### Rust in rust

| Tool | Files | Code lines | Time |
| ---- | ----: | ---------: | ---: |
| ripcloc | 39,811 | 2,997,997 | **67.6ms ± 1.9ms** (1.00x) |
| [scc](https://github.com/boyter/scc) | 39,805 | 3,027,133 | 126.3ms ± 2.6ms (1.87x) |
| [tokei](https://github.com/XAMPPRocky/tokei) | 39,811 | 3,034,074 | 173.9ms ± 3.5ms (2.57x) |

#### JavaScript and TypeScript in react

| Tool | Files | Code lines | Time |
| ---- | ----: | ---------: | ---: |
| ripcloc | 4,449 | 604,524 | **12.7ms ± 0.6ms** (1.00x) |
| [scc](https://github.com/boyter/scc) | 4,452 | 607,682 | 26.4ms ± 1.5ms (2.07x) |
| [tokei](https://github.com/XAMPPRocky/tokei) | 4,449 | 606,993 | 28.3ms ± 1.3ms (2.22x) |

### Default settings, in the reverse order

#### One small Rust file

| Tool | Files | Code lines | Time |
| ---- | ----: | ---------: | ---: |
| ripcloc `--no-attribution` | 1 | 335 | **1.1ms ± 0.3ms** (1.00x) |
| ripcloc | 1 | 335 | 1.3ms ± 0.2ms (1.19x) |
| [tokei](https://github.com/XAMPPRocky/tokei) | 1 | 335 | 4.3ms ± 0.5ms (3.84x) |
| [scc](https://github.com/boyter/scc) | 1 | 335 | 7.5ms ± 0.3ms (6.79x) |

#### One large Rust file, avx512f.rs

| Tool | Files | Code lines | Time |
| ---- | ----: | ---------: | ---: |
| ripcloc `--no-attribution` | 1 | 48,200 | **3.9ms ± 0.3ms** (1.00x) |
| ripcloc | 1 | 48,200 | 7.7ms ± 0.3ms (1.99x) |
| [scc](https://github.com/boyter/scc) | 1 | 48,200 | 17.4ms ± 1.0ms (4.51x) |
| [tokei](https://github.com/XAMPPRocky/tokei) | 1 | 48,200 | 17.5ms ± 0.7ms (4.54x) |

#### react

| Tool | Files | Code lines | Time |
| ---- | ----: | ---------: | ---: |
| ripcloc `--no-attribution` | 6,962 | 830,286 | **21.2ms ± 1.4ms** (1.00x) |
| ripcloc | 6,962 | 830,286 | 22.8ms ± 1.5ms (1.08x) |
| [tokei](https://github.com/XAMPPRocky/tokei) | 6,988 | 778,235 | 44.3ms ± 1.0ms (2.10x) |
| [scc](https://github.com/boyter/scc) | 7,058 | 825,668 | 47.5ms ± 1.5ms (2.24x) |

#### rails

| Tool | Files | Code lines | Time |
| ---- | ----: | ---------: | ---: |
| ripcloc `--no-attribution` | 4,516 | 517,161 | **15.4ms ± 0.7ms** (1.00x) |
| ripcloc | 4,516 | 517,161 | 16.4ms ± 0.7ms (1.06x) |
| [scc](https://github.com/boyter/scc) | 4,745 | 522,842 | 29.5ms ± 1.7ms (1.91x) |
| [tokei](https://github.com/XAMPPRocky/tokei) | 4,566 | 463,934 | 33.3ms ± 1.4ms (2.16x) |

#### rust

| Tool | Files | Code lines | Time |
| ---- | ----: | ---------: | ---: |
| ripcloc `--no-attribution` | 41,776 | 4,067,691 | **89.0ms ± 1.9ms** (1.00x) |
| ripcloc | 41,776 | 4,067,691 | 107.3ms ± 2.5ms (1.21x) |
| [scc](https://github.com/boyter/scc) | 44,331 | 4,216,656 | 175.7ms ± 2.7ms (1.97x) |
| [tokei](https://github.com/XAMPPRocky/tokei) | 43,196 | 4,036,065 | 278.5ms ± 15.3ms (3.13x) |

#### linux

| Tool | Files | Code lines | Time |
| ---- | ----: | ---------: | ---: |
| ripcloc `--no-attribution` | 89,872 | 33,293,243 | **216.2ms ± 7.0ms** (1.00x) |
| ripcloc | 89,872 | 33,293,243 | 231.9ms ± 8.7ms (1.07x) |
| [scc](https://github.com/boyter/scc) | 91,228 | 33,464,408 | 650.0ms ± 10.1ms (3.01x) |
| [tokei](https://github.com/XAMPPRocky/tokei) | 91,270 | 33,396,065 | 794.5ms ± 7.5ms (3.67x) |
