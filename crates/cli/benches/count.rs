// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! Counts generated sources of fixed content many times over, so that the instructions of a run
//! are nearly all those of counting. `bench/instructions` builds and runs it, and reads the
//! instruction counts of its runs from outside the process.
//!
//! The sources are mostly plain lines of code, with about the shares of blank lines, comments and
//! strings that the code of Linux, React and Rust has, and some of the Rust sources end with a
//! test module.
//!
//! `count <input>` counts the input that is named, and `count none` only generates the inputs, so
//! that the instructions of an input are those of its run less those of that one. Without a name,
//! every input is counted once and its counts are printed.

use std::hint::black_box;

#[allow(dead_code)]
#[path = "../src/cloc.rs"]
mod cloc;

#[allow(dead_code)]
#[path = "../src/cfg_test.rs"]
mod cfg_test;

use cloc::LineCounts;

/// The number of files in each input, which makes each input a few hundred kilobytes.
const FILES: usize = 40;

/// The number of functions in each file.
const FUNCTIONS: usize = 25;

/// The number of times that a named input is counted, so that counting takes most of the
/// instructions of a run rather than starting the process and generating the inputs does.
const PASSES: usize = 50;

/// The inputs, by name.
const INPUTS: [&str; 3] = ["c", "js", "rs"];

/// A C source in the style of the Linux kernel.
fn c_source(file: usize) -> String {
    let mut text = format!(
        "// SPDX-License-Identifier: GPL-2.0\n/*\n * Sums of file {file}.\n */\n\n\
         #include <linux/kernel.h>\n#include <linux/slab.h>\n\n"
    );
    for i in 0..FUNCTIONS {
        if i.is_multiple_of(3) {
            text += &format!("/*\n * sum_{i} - add up the first @len values\n */\n");
        }
        text += &format!(
            "static int sum_{i}(const int *values, size_t len)\n{{\n\tint total = 0;\n\
             \tsize_t j;\n\n\tfor (j = 0; j < len; j++)\n\t\ttotal += values[j] * {i};\n\
             \tif (total < 0)\n\t\tpr_warn(\"sum_{i}: overflow\\n\");\n\treturn total;\n}}\n\n"
        );
    }
    text
}

/// A JavaScript source in the style of React.
fn js_source(file: usize) -> String {
    let mut text = format!(
        "'use strict';\n\nimport {{useState}} from 'react';\nimport {{render}} from './render{file}';\n\n"
    );
    for i in 0..FUNCTIONS {
        text += &format!("// Renders the items of list {i}.\n");
        text += &format!(
            "export function List{i}({{items, onSelect}}) {{\n\
             \x20 const [selected, setSelected] = useState(null);\n\
             \x20 const visible = items.filter(item => item.id !== {i});\n\
             \x20 if (visible.length === 0) {{\n    return null;\n  }}\n\
             \x20 return render(visible, item => {{\n    setSelected(item);\n\
             \x20   onSelect(`item ${{item.id}}`);\n  }});\n}}\n\n"
        );
    }
    text
}

/// A Rust source, which ends with a test module in every other file.
fn rust_source(file: usize) -> String {
    let mut text = format!("//! The sums of file {file}.\n\nuse std::collections::HashMap;\n\n");
    for i in 0..FUNCTIONS {
        text += &format!(
            "/// The sum of the values of `map` for keys below {i}.\n#[inline]\n\
             pub fn sum_{i}(map: &HashMap<u32, u64>) -> u64 {{\n\
             \x20   let mut total = 0;\n    for (&key, &value) in map {{\n\
             \x20       if key < {i} {{\n            total += value;\n        }}\n    }}\n\
             \x20   // Wrap on overflow.\n    total.wrapping_mul({i})\n}}\n\n"
        );
    }
    if file.is_multiple_of(2) {
        text += "#[cfg(test)]\nmod tests {\n    use super::*;\n";
        for i in 0..FUNCTIONS / 2 {
            text += &format!(
                "\n    #[test]\n    fn sums_{i}() {{\n        let map = HashMap::from([(1, 2)]);\n\
                 \x20       assert_eq!(sum_{i}(&map), 0, \"sum_{i} of {{map:?}}\");\n    }}\n"
            );
        }
        text += "}\n";
    }
    text
}

/// The counts of the input named `input`, which is also the extension of its files, and the
/// counts of its test modules.
fn count(input: &str, sources: &Sources) -> Option<(LineCounts, LineCounts)> {
    let files = match input {
        "c" => &sources.c,
        "js" => &sources.js,
        "rs" => &sources.rust,
        _ => return None,
    };
    let (mut counts, mut test) = (LineCounts::default(), LineCounts::default());
    for file in files {
        let (lines, tests) = match input {
            "rs" => cfg_test::count(black_box(file.as_bytes())),
            _ => (cloc::count_lines(black_box(file), input), LineCounts::default()),
        };
        counts.add(lines);
        test.add(tests);
    }
    Some((counts, test))
}

/// The generated inputs, each a list of files.
struct Sources {
    c: Vec<String>,
    js: Vec<String>,
    rust: Vec<String>,
}

fn main() {
    let sources = Sources {
        c: (0..FILES).map(c_source).collect(),
        js: (0..FILES).map(js_source).collect(),
        rust: (0..FILES).map(rust_source).collect(),
    };
    // `cargo bench` passes flags such as `--bench`, which are not inputs.
    let inputs: Vec<String> =
        std::env::args().skip(1).filter(|arg| !arg.starts_with('-')).collect();
    if inputs.is_empty() {
        for input in INPUTS {
            if let Some((counts, test)) = count(input, &sources) {
                println!("{input:8} {counts:?} with tests {test:?}");
            }
        }
        return;
    }
    for input in &inputs {
        if input == "none" {
            continue;
        }
        for _ in 0..PASSES {
            let Some(counts) = count(input, &sources) else {
                eprintln!("count: {input} is not an input, and the inputs are c, js and rs");
                std::process::exit(2);
            };
            black_box(counts);
        }
    }
}
