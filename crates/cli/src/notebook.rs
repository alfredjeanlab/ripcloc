// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

//! Line counting for Jupyter notebooks, whose cells are counted rather than the JSON that holds
//! them.
//!
//! A code cell is counted in the language of the notebook's kernel, and a Markdown cell is prose,
//! whose lines count as comments, since they explain the code as comments do. Raw cells are not
//! counted, and neither are outputs, which can hold megabytes of images. The JSON is read with a
//! deserializer that keeps only the sources of cells and the name of the kernel's language, so
//! that an output is skipped without being copied.

use std::fmt;
use std::marker::PhantomData;

use serde_core::de::{Deserialize, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};

use crate::syntax::{self, LineCounts};

/// Count the lines of the cells of the notebook `content`, or return `None` if it is not a
/// notebook in JSON.
///
/// The kernel's language is read from `metadata.language_info.name` or else from
/// `metadata.kernelspec.language`, and is Python when the notebook names neither. A language that
/// is not counted has no comment syntax, so every line of its code is code. A notebook in the
/// format of version 3, which keeps its cells in worksheets and the source of a code cell in its
/// `input`, is read as well.
pub fn count(content: &[u8]) -> Option<LineCounts> {
    let content = content.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(content);
    let notebook: Notebook = serde_json::from_slice(content).ok()?;
    let language = notebook.metadata.language_info.name.or(notebook.metadata.kernelspec.language);
    let key = language.as_deref().map_or("py", kernel_key);
    let mut counts = LineCounts::default();
    for cell in &notebook.cells {
        match cell.cell_type.as_str() {
            "code" => counts.add(syntax::count_in(cell.source.as_bytes(), key, None)),
            "markdown" | "heading" => counts.add(prose(cell.source.as_bytes())),
            _ => {}
        }
    }
    Some(counts)
}

/// The key of the language that a kernel names, such as `python` or `C++17`.
fn kernel_key(language: &str) -> &'static str {
    let language = language.to_ascii_lowercase();
    match language.as_str() {
        "python" | "python2" | "python3" | "ipython" | "ipython3" => "py",
        "r" => "r",
        "julia" => "jl",
        "scala" => "scala",
        "rust" => "rs",
        "javascript" | "node.js" => "js",
        "typescript" => "ts",
        "ruby" => "rb",
        "bash" | "sh" | "shell" => "sh",
        "go" => "go",
        "haskell" => "hs",
        "ocaml" => "ml",
        "kotlin" => "kt",
        "java" => "java",
        "c" => "c",
        "c#" | "csharp" => "cs",
        "f#" | "fsharp" => "fs",
        "sql" => "sql",
        "lua" => "lua",
        "perl" => "pl",
        "powershell" => "ps1",
        "elixir" => "ex",
        "clojure" => "clj",
        "groovy" => "groovy",
        "swift" => "swift",
        "prolog" => "prolog",
        // xeus-cling names the standard, as in `C++17`.
        other if other.starts_with("c++") => "cpp",
        _ => "",
    }
}

/// Count the lines of `text`, prose in which every line that is not blank is a comment.
fn prose(text: &[u8]) -> LineCounts {
    let mut counts = LineCounts::default();
    for line in syntax::lines(text) {
        if syntax::trim(line).is_empty() {
            counts.blank += 1;
        } else {
            counts.comment += 1;
        }
    }
    counts
}

/// The parts of a notebook that are counted.
#[derive(Default)]
struct Notebook {
    cells: Vec<Cell>,
    metadata: Metadata,
}

#[derive(Default)]
struct Metadata {
    language_info: Language,
    kernelspec: Language,
}

/// The `language_info` or the `kernelspec` of a notebook's metadata, whose `name` and `language`
/// are kept.
#[derive(Default)]
struct Language {
    name: Option<String>,
    language: Option<String>,
}

/// A worksheet of a notebook in the format of version 3.
#[derive(Default)]
struct Worksheet {
    cells: Vec<Cell>,
}

#[derive(Default)]
struct Cell {
    cell_type: String,
    source: String,
}

/// A JSON object, read field by field into a value that starts as its default.
trait Object: Default {
    /// Read the value of the field `key` from `map` into `self`. A field that is not kept must
    /// still be read, as an [`IgnoredAny`].
    fn field<'de, A: MapAccess<'de>>(&mut self, key: &str, map: &mut A) -> Result<(), A::Error>;
}

impl Object for Notebook {
    fn field<'de, A: MapAccess<'de>>(&mut self, key: &str, map: &mut A) -> Result<(), A::Error> {
        match key {
            "cells" => self.cells.extend(map.next_value::<Vec<Cell>>()?),
            "worksheets" => {
                let worksheets = map.next_value::<Vec<Worksheet>>()?;
                self.cells.extend(worksheets.into_iter().flat_map(|w| w.cells));
            }
            "metadata" => self.metadata = map.next_value()?,
            _ => _ = map.next_value::<IgnoredAny>()?,
        }
        Ok(())
    }
}

impl Object for Metadata {
    fn field<'de, A: MapAccess<'de>>(&mut self, key: &str, map: &mut A) -> Result<(), A::Error> {
        match key {
            "language_info" => self.language_info = map.next_value()?,
            "kernelspec" => self.kernelspec = map.next_value()?,
            _ => _ = map.next_value::<IgnoredAny>()?,
        }
        Ok(())
    }
}

impl Object for Language {
    fn field<'de, A: MapAccess<'de>>(&mut self, key: &str, map: &mut A) -> Result<(), A::Error> {
        match key {
            "name" => self.name = map.next_value()?,
            "language" => self.language = map.next_value()?,
            _ => _ = map.next_value::<IgnoredAny>()?,
        }
        Ok(())
    }
}

impl Object for Worksheet {
    fn field<'de, A: MapAccess<'de>>(&mut self, key: &str, map: &mut A) -> Result<(), A::Error> {
        match key {
            "cells" => self.cells = map.next_value()?,
            _ => _ = map.next_value::<IgnoredAny>()?,
        }
        Ok(())
    }
}

impl Object for Cell {
    fn field<'de, A: MapAccess<'de>>(&mut self, key: &str, map: &mut A) -> Result<(), A::Error> {
        match key {
            "cell_type" => self.cell_type = map.next_value()?,
            "source" | "input" => self.source = map.next_value::<Source>()?.0,
            _ => _ = map.next_value::<IgnoredAny>()?,
        }
        Ok(())
    }
}

/// Implements [`Deserialize`] for each type that is read as an [`Object`].
macro_rules! deserialize_object {
    ($($t:ty),*) => {$(
        impl<'de> Deserialize<'de> for $t {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                deserializer.deserialize_map(ObjectVisitor(PhantomData))
            }
        }
    )*};
}

deserialize_object!(Notebook, Metadata, Language, Worksheet, Cell);

struct ObjectVisitor<T>(PhantomData<T>);

impl<'de, T: Object> Visitor<'de> for ObjectVisitor<T> {
    type Value = T;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<T, A::Error> {
        let mut value = T::default();
        while let Some(key) = map.next_key::<String>()? {
            value.field(&key, &mut map)?;
        }
        Ok(value)
    }
}

/// The source of a cell, which is a string or an array of strings, each a line that keeps its
/// newline.
struct Source(String);

impl<'de> Deserialize<'de> for Source {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(SourceVisitor)
    }
}

struct SourceVisitor;

impl<'de> Visitor<'de> for SourceVisitor {
    type Value = Source;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a string or an array of strings")
    }

    fn visit_str<E>(self, text: &str) -> Result<Source, E> {
        Ok(Source(text.to_owned()))
    }

    fn visit_string<E>(self, text: String) -> Result<Source, E> {
        Ok(Source(text))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Source, A::Error> {
        let mut text = String::new();
        while let Some(line) = seq.next_element::<std::borrow::Cow<'de, str>>()? {
            text.push_str(&line);
        }
        Ok(Source(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(json: &str) -> Option<(usize, usize, usize)> {
        count(json.as_bytes()).map(|c| (c.blank, c.comment, c.code))
    }

    #[test]
    fn counts_code_cells_in_the_kernels_language_and_markdown_as_comments() {
        let json = r##"{
 "cells": [
  {"cell_type": "markdown", "metadata": {}, "source": ["# Title\n", "\n", "Some prose.\n"]},
  {"cell_type": "code", "execution_count": 1, "metadata": {},
   "outputs": [{"output_type": "display_data", "data": {"image/png": "iVBORw0KGgo=\n# not counted"}}],
   "source": ["# a comment\n", "x = 1\n", "\n", "print(x)"]},
  {"cell_type": "raw", "metadata": {}, "source": "raw text\n"}
 ],
 "metadata": {"kernelspec": {"name": "python3", "language": "python"},
              "language_info": {"name": "python"}},
 "nbformat": 4, "nbformat_minor": 5
}"##;
        assert_eq!(counts(json), Some((2, 3, 2)));
    }

    #[test]
    fn falls_back_to_the_kernelspec_and_then_to_python() {
        let cell = r#""cells": [{"cell_type": "code", "source": "// c\n# d\nx\n"}]"#;
        let rust = format!(r#"{{{cell}, "metadata": {{"kernelspec": {{"language": "Rust"}}}}}}"#);
        assert_eq!(counts(&rust), Some((0, 1, 2)));
        let python = format!("{{{cell}}}");
        assert_eq!(counts(&python), Some((0, 1, 2)));
        let unknown = format!(r#"{{{cell}, "metadata": {{"language_info": {{"name": "q"}}}}}}"#);
        assert_eq!(counts(&unknown), Some((0, 0, 3)));
        let cpp = format!(r#"{{{cell}, "metadata": {{"language_info": {{"name": "C++17"}}}}}}"#);
        assert_eq!(counts(&cpp), Some((0, 1, 2)));
    }

    #[test]
    fn reads_worksheets_of_version_3() {
        let json = r##"{"worksheets": [{"cells": [
            {"cell_type": "code", "language": "python", "input": ["# c\n", "x = 1\n"]},
            {"cell_type": "heading", "level": 1, "source": ["Title"]}
        ]}], "metadata": {}, "nbformat": 3}"##;
        assert_eq!(counts(json), Some((0, 2, 1)));
    }

    #[test]
    fn a_malformed_notebook_is_not_counted() {
        assert_eq!(counts(""), None);
        assert_eq!(counts("{\"cells\": ["), None);
        assert_eq!(counts("[1, 2]"), None);
        assert_eq!(counts(r#"{"cells": [{"cell_type": "code", "source": 3}]}"#), None);
        assert_eq!(counts("{}"), Some((0, 0, 0)));
    }
}
