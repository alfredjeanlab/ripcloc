// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! `ripcloc`, which counts lines of code very fast.

mod args;
mod color;
mod report;

use std::io::{ErrorKind, Write};

use ripcloc::Options;

use args::Args;

fn main() -> anyhow::Result<()> {
    let mut args = Args::parse();
    // The listing of `--languages` belongs here, before any path is read.
    if args.languages {
        return write_report(&report::languages(args.output)?);
    }
    if args.compact {
        args.no_attribution = true;
        args.depth = args.depth.or(Some(0));
    }
    let options = Options {
        max_bytes: max_bytes(&args),
        attribution: !args.no_attribution,
        deps: args.deps,
        ignore: !args.no_ignore,
        max_depth: args.max_depth,
    };
    let counts = ripcloc::count(&args.paths, &options)?;
    write_report(&report::counts(&counts, &args)?)
}

/// The size in bytes above which a file is not counted.
fn max_bytes(args: &Args) -> u64 {
    const MB: u64 = 1024 * 1024;
    match args.max_file_size {
        Some(mb) => mb.saturating_mul(MB),
        None if args.large => u64::MAX,
        None if args.no_large => 10 * MB,
        None => 1024 * MB,
    }
}

/// Write `report` to standard output at once, so that a reader that closes early is handled in
/// one place.
fn write_report(report: &str) -> anyhow::Result<()> {
    let mut stdout = std::io::stdout().lock();
    match stdout.write_all(report.as_bytes()).and_then(|()| stdout.flush()) {
        // A reader that stops early, such as `head`, has read all that it wanted.
        Err(e) if e.kind() == ErrorKind::BrokenPipe => Ok(()),
        written => Ok(written?),
    }
}
