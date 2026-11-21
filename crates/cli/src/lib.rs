// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! The counting behind the `ripcloc` command. It is a library so that the benchmark can call the
//! counters directly, and its API is not stable.

pub mod cfg_test;
mod count;
mod lang;
mod notebook;
mod packages;
mod project;
pub mod syntax;

pub use count::{Counts, Kind, Options, Row, Rows, Stats, count};
