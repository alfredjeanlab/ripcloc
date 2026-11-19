"""Prints a benchmark's results from bench/run as a Markdown table, fastest first, in the style of ripgrep's README."""

import json
import sys
from pathlib import Path

links = {
    "ripcloc": "ripcloc",
    "ripcloc-noattr": "ripcloc `--no-attribution`",
    "tokei": "[tokei](https://github.com/XAMPPRocky/tokei)",
    "scc": "[scc](https://github.com/boyter/scc)",
    "gocloc": "[gocloc](https://github.com/hhatto/gocloc)",
    "loc": "[loc](https://github.com/cgag/loc)",
    "cloc": "[cloc](https://github.com/AlDanial/cloc)",
}


titles = {
    "large_c": "One large C file, sqlite3.c",
    "large_rust": "One large Rust file, avx512f.rs",
    "large_ts": "One large TypeScript file, checker.ts",
    "small_file": "One small Rust file",
    "ripcloc": "ripcloc",
    "cloc": "cloc",
    "grep": "GNU grep 3.12",
    "ripgrep": "ripgrep",
    "react": "react",
    "rails": "rails",
    "rust": "rust",
    "linux": "linux",
    "linux_ch": "C and headers in linux",
    "rust_rs": "Rust in rust",
    "react_js": "JavaScript and TypeScript in react",
}


def seconds(s):
    return f"{s * 1000:.1f}ms" if s < 1 else f"{s:.3f}s"


out = Path(sys.argv[1])
for bench in sys.argv[2:]:
    counts = {}
    for line in (out / f"{bench}.counts").read_text().splitlines():
        tool, files, code = line.split()
        counts[tool] = (files, code)
    results = sorted(json.loads((out / f"{bench}.json").read_text())["results"], key=lambda r: r["mean"])
    fastest = results[0]["mean"]
    print(f"### {titles.get(bench, bench)}\n")
    print("| Tool | Files | Code lines | Time |")
    print("| ---- | ----: | ---------: | ---: |")
    for i, r in enumerate(results):
        files, code = counts[r["command"]]
        time = f"{seconds(r['mean'])} ± {seconds(r['stddev'])}"
        if i == 0:
            time = f"**{time}**"
        files, code = (f"{int(n):,}" if n.isdigit() else n for n in (files, code))
        print(f"| {links[r['command']]} | {files} | {code} | {time} ({r['mean'] / fastest:.2f}x) |")
    print()
