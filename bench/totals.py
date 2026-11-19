"""Reads a tool's default text output on stdin and prints `<tool> <files> <code>` from its total row."""

import json
import re
import sys

tool = sys.argv[1]
# The label of each tool's total row and the positions of files and code among the numbers that follow it.
rows = {
    "ripcloc": ("Total", 0, 4),
    "tokei": ("Total", 0, 2),
    "scc": ("Total", 0, 4),
    "gocloc": ("TOTAL", 0, 3),
    "loc": ("Total", 0, 4),
}
if tool == "cloc":
    total = json.load(sys.stdin)["SUM"]
    print(tool, total["nFiles"], total["code"])
    sys.exit()
label, files, code = rows[tool.removesuffix("-noattr")]
total = None
for line in sys.stdin:
    words = line.split()
    if words and words[0] == label:
        total = [int(w.replace(",", "")) for w in words[1:] if re.fullmatch(r"[\d,]+", w)]
if total is None:
    print(tool, "?", "?")
else:
    print(tool, total[files], total[code])
