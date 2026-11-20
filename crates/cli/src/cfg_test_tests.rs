// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Alfred Jean LLC

use super::*;

/// The inline test modules of a Rust source file.
struct CfgTestInfo {
    /// Line ranges (0-indexed) of `#[cfg(test)] mod` blocks, from the attribute through the
    /// closing brace.
    test_ranges: Vec<Range<usize>>,
}

impl CfgTestInfo {
    /// Find the test modules of `content`, and check that they are the same whether lexing skips
    /// to the lines that counting pushes or lexes every line.
    fn parse(content: &(impl AsRef<[u8]> + ?Sized)) -> Self {
        let content = content.as_ref();
        let mut code_starts = Vec::new();
        cloc::count_in(content, "rs", Some(&mut code_starts));
        let spans = test_spans(content, &[]);
        assert_eq!(test_spans(content, &code_starts), spans);
        // The index of the line that starts at `offset`, or of the line after the last at the end.
        let line = |offset: usize| {
            let unterminated = offset == content.len() && !content.ends_with(b"\n");
            memchr_iter(b'\n', content.get(..offset).unwrap_or_default()).count()
                + usize::from(unterminated)
        };
        Self { test_ranges: spans.into_iter().map(|s| line(s.start)..line(s.end)).collect() }
    }

    fn is_test_line(&self, line_idx: usize) -> bool {
        self.test_ranges.iter().any(|r| r.contains(&line_idx))
    }
}

/// The line ranges (0-indexed) of the inline test modules of a Rust source file.
fn test_ranges(content: &[u8]) -> Vec<Range<usize>> {
    CfgTestInfo::parse(content).test_ranges
}

/// The counts of `content` and of its test modules, found by lexing every line.
fn counted_alone(content: &str) -> (LineCounts, LineCounts) {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut test = LineCounts::default();
    for span in test_spans(content.as_bytes(), &[]) {
        test.add(cloc::count_lines(&content[span], "rs"));
    }
    (cloc::count_lines(content, "rs"), test)
}

#[test]
fn basic_cfg_test_block() {
    let content = r#"
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_add() {
        assert_eq!(super::add(1, 2), 3);
    }
}
"#;
    let info = CfgTestInfo::parse(content);

    // Lines 0-4 are source (empty, pub fn, a+b, }, empty)
    // Lines 5-11 are test (#[cfg(test)], mod tests, #[test], fn, assert, }, })
    assert!(!info.is_test_line(1)); // pub fn add
    assert!(!info.is_test_line(2)); // a + b
    assert!(info.is_test_line(5)); // #[cfg(test)]
    assert!(info.is_test_line(6)); // mod tests
    assert!(info.is_test_line(11)); // closing brace
}

#[test]
fn nested_braces_in_test() {
    let content = r#"
pub fn main() {}

#[cfg(test)]
mod tests {
    fn helper() {
        if true {
            println!("nested");
        }
    }
}
"#;
    let info = CfgTestInfo::parse(content);

    assert!(!info.is_test_line(1)); // pub fn main
    assert!(info.is_test_line(3)); // #[cfg(test)]
    assert!(info.is_test_line(7)); // nested println
    assert!(info.is_test_line(10)); // closing brace of mod tests
}

#[test]
fn multiple_cfg_test_blocks() {
    let content = r#"
fn a() {}

#[cfg(test)]
mod tests_a {
    #[test]
    fn test_a() {}
}

fn b() {}

#[cfg(test)]
mod tests_b {
    #[test]
    fn test_b() {}
}
"#;
    let info = CfgTestInfo::parse(content);

    assert_eq!(info.test_ranges.len(), 2);
    assert!(!info.is_test_line(1)); // fn a()
    assert!(info.is_test_line(3)); // first #[cfg(test)]
    assert!(!info.is_test_line(9)); // fn b()
    assert!(info.is_test_line(11)); // second #[cfg(test)]
}

#[test]
fn no_cfg_test_blocks() {
    let content = r#"
pub fn main() {
    println!("Hello");
}
"#;
    let info = CfgTestInfo::parse(content);

    assert!(info.test_ranges.is_empty());
    assert!(!info.is_test_line(0));
    assert!(!info.is_test_line(1));
}

#[test]
fn cfg_test_with_spaces() {
    // #[cfg(test)] with extra whitespace inside
    let content = r#"
pub fn main() {}

#[cfg( test )]
mod tests {
    fn test() {}
}
"#;
    let info = CfgTestInfo::parse(content);

    assert!(!info.test_ranges.is_empty());
    assert!(info.is_test_line(3)); // #[cfg( test )]
}

#[test]
fn string_literals_with_braces() {
    // Test that braces in string literals are correctly skipped
    let content = r#"
fn source() {}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
        let s = "{ not a real brace }";
        assert!(true);
    }
}
"#;
    let info = CfgTestInfo::parse(content);

    // With improved string handling, this should parse correctly
    assert_eq!(info.test_ranges.len(), 1);
    assert!(info.is_test_line(3)); // #[cfg(test)]
    assert!(info.is_test_line(10)); // closing brace of mod tests
}

#[test]
fn escaped_quotes_in_strings() {
    // Test that escaped quotes don't confuse the parser
    let content = r#"
fn source() {}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
        let s = "hello \"world\" {}";
        assert!(true);
    }
}
"#;
    let info = CfgTestInfo::parse(content);

    assert_eq!(info.test_ranges.len(), 1);
    assert!(info.is_test_line(10)); // closing brace
}

#[test]
fn raw_string_with_braces() {
    // Raw strings containing braces should not affect brace counting
    let content = r#"
fn source() {}

#[cfg(test)]
mod tests {
    #[test]
    fn test_raw_string() {
        let s = r"{ not a real brace }";
        assert!(true);
    }
}
"#;
    let info = CfgTestInfo::parse(content);

    assert_eq!(info.test_ranges.len(), 1);
    assert!(info.is_test_line(3)); // #[cfg(test)]
    assert!(info.is_test_line(10)); // closing brace of mod tests
}

#[test]
fn raw_string_with_hashes() {
    // Raw strings with hash delimiters
    let content = r###"
fn source() {}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
        let s = r#"{ braces } and "quotes""#;
        let t = r##"more { braces }"##;
        assert!(true);
    }
}
"###;
    let info = CfgTestInfo::parse(content);

    assert_eq!(info.test_ranges.len(), 1);
    assert!(info.is_test_line(3)); // #[cfg(test)]
    assert!(info.is_test_line(11)); // closing brace
}

#[test]
fn char_literal_with_brace() {
    // Character literals containing braces
    let content = r#"
fn source() {}

#[cfg(test)]
mod tests {
    #[test]
    fn test_char() {
        let open = '{';
        let close = '}';
        assert_eq!(open, '{');
    }
}
"#;
    let info = CfgTestInfo::parse(content);

    assert_eq!(info.test_ranges.len(), 1);
    assert!(info.is_test_line(3)); // #[cfg(test)]
    assert!(info.is_test_line(11)); // closing brace
}

#[test]
fn char_literal_with_escaped_quote() {
    // Escaped quote in char literal shouldn't confuse parser
    let content = r#"
fn source() {}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
        let quote = '\'';
        let brace = '{';
        assert!(true);
    }
}
"#;
    let info = CfgTestInfo::parse(content);

    assert_eq!(info.test_ranges.len(), 1);
    assert!(info.is_test_line(10)); // closing brace
}

#[test]
fn mixed_string_types() {
    // Mix of regular strings, raw strings, and char literals
    let content = r###"
fn source() {}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
        let a = "{ regular }";
        let b = r"{ raw }";
        let c = r#"{ raw # }"#;
        let d = '{';
        let e = '}';
        assert!(true);
    }
}
"###;
    let info = CfgTestInfo::parse(content);

    assert_eq!(info.test_ranges.len(), 1);
    assert!(info.is_test_line(14)); // closing brace
}

#[test]
fn lifetime_not_confused_with_char() {
    // Lifetimes should not be confused with char literals
    let content = r#"
fn source<'a>(x: &'a str) -> &'a str { x }

#[cfg(test)]
mod tests {
    fn helper<'a>(x: &'a str) -> &'a str {
        x
    }
}
"#;
    let info = CfgTestInfo::parse(content);

    assert_eq!(info.test_ranges.len(), 1);
    assert!(!info.is_test_line(1)); // source function
    assert!(info.is_test_line(3)); // #[cfg(test)]
}

#[test]
fn nested_raw_strings() {
    // Raw strings can contain quote characters
    let content = r####"
fn source() {}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
        let s = r#"contains "quotes" and { braces }"#;
        assert!(true);
    }
}
"####;
    let info = CfgTestInfo::parse(content);

    assert_eq!(info.test_ranges.len(), 1);
}

#[test]
fn empty_string_and_char() {
    // Edge case: empty-ish strings
    let content = r#"
fn source() {}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
        let s = "";
        let c = ' ';
        let brace_str = "{}";
        assert!(true);
    }
}
"#;
    let info = CfgTestInfo::parse(content);

    assert_eq!(info.test_ranges.len(), 1);
}

// =============================================================================
// MULTI-LINE ATTRIBUTE TESTS
// =============================================================================

#[test]
fn multiline_cfg_test_attribute() {
    // Multi-line #[cfg(test)] should be detected
    let content = r#"
pub fn source_code() -> i32 {
    42
}

#[cfg(
    test
)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(super::source_code(), 42);
    }
}
"#;
    let info = CfgTestInfo::parse(content);

    assert_eq!(info.test_ranges.len(), 1, "should detect multi-line cfg(test)");
    assert!(!info.is_test_line(1)); // pub fn source_code
    assert!(info.is_test_line(5)); // #[cfg(
    assert!(info.is_test_line(8)); // mod tests
    assert!(info.is_test_line(13)); // closing brace
}

#[test]
fn multiline_cfg_test_with_extra_whitespace() {
    // Multi-line with lots of whitespace
    let content = r#"
fn main() {}

#[cfg(
        test
    )]
mod tests {
    fn test() {}
}
"#;
    let info = CfgTestInfo::parse(content);

    assert_eq!(info.test_ranges.len(), 1);
    assert!(!info.is_test_line(1)); // fn main
    assert!(info.is_test_line(3)); // #[cfg(
}

#[test]
fn multiline_cfg_not_test() {
    // Multi-line #[cfg(...)] that is NOT test
    let content = r#"
fn always() {}

#[cfg(
    feature = "foo"
)]
fn feature_foo() {}

#[cfg(test)]
mod tests {
    fn test() {}
}
"#;
    let info = CfgTestInfo::parse(content);

    // Should only have 1 range (the actual cfg(test) block)
    assert_eq!(info.test_ranges.len(), 1);
    assert!(!info.is_test_line(1)); // fn always
    assert!(!info.is_test_line(6)); // fn feature_foo
    assert!(info.is_test_line(8)); // #[cfg(test)]
}

#[test]
fn multiline_cfg_all_test() {
    // Multi-line #[cfg(all(test, ...))] should be detected
    let content = r#"
fn source() {}

#[cfg(all(
    test,
    feature = "integration"
))]
mod integration_tests {
    fn test() {}
}
"#;
    let info = CfgTestInfo::parse(content);

    assert_eq!(info.test_ranges.len(), 1);
    assert!(!info.is_test_line(1)); // fn source
    assert!(info.is_test_line(3)); // #[cfg(all(
}

#[test]
fn doc_comment_mentioning_cfg_test_not_detected() {
    // Doc comments that mention #[cfg(test)] should not be detected as cfg(test) blocks
    let content = r#"
/// Mode for handling #[cfg(test)] blocks in Rust files.
#[derive(Debug, Clone, Copy)]
pub enum CfgTestSplitMode {
    Count,
    Require,
}
"#;
    let info = CfgTestInfo::parse(content);

    // Should have no test blocks
    assert_eq!(info.test_ranges.len(), 0);
    assert!(info.test_ranges.is_empty());
}

#[test]
fn line_comment_mentioning_cfg_test_not_detected() {
    // Regular line comments that mention #[cfg(test)] should not be detected
    let content = r#"
fn process() {
    // Check if line is in test code (file-level OR inline #[cfg(test)])
    let is_test = false;
}
"#;
    let info = CfgTestInfo::parse(content);

    // Should have no test blocks
    assert_eq!(info.test_ranges.len(), 0);
    assert!(info.test_ranges.is_empty());
}

#[test]
fn block_comment_mentioning_cfg_test_not_detected() {
    // Block comments that mention #[cfg(test)] should not be detected
    let content = r#"
fn process() {
    /* This function handles #[cfg(test)] blocks
       and other conditional compilation */
    let x = 42;
}
"#;
    let info = CfgTestInfo::parse(content);

    // Should have no test blocks
    assert_eq!(info.test_ranges.len(), 0);
    assert!(info.test_ranges.is_empty());
}

#[test]
fn multiline_block_comment_with_cfg_test_not_detected() {
    // Multi-line block comments spanning several lines should be ignored
    let content = r#"
/*
 * Documentation about test handling:
 * - Use #[cfg(test)] for test modules
 * - Keep tests in separate files
 */
fn main() {}
"#;
    let info = CfgTestInfo::parse(content);

    // Should have no test blocks
    assert_eq!(info.test_ranges.len(), 0);
    assert!(info.test_ranges.is_empty());
}

// =============================================================================
// ITEM KIND DETECTION TESTS
// =============================================================================

#[test]
fn item_kind_mod() {
    let content = r#"
fn source() {}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
"#;
    let info = CfgTestInfo::parse(content);
    assert_eq!(info.test_ranges.len(), 1);
}

#[test]
fn item_kind_pub_mod() {
    let content = r#"
#[cfg(test)]
pub mod tests {
    fn test() {}
}
"#;
    let info = CfgTestInfo::parse(content);
    assert_eq!(info.test_ranges.len(), 1);
}

#[test]
fn item_kind_fn() {
    let content = r#"
#[cfg(test)]
fn test_helper() -> i32 {
    42
}
"#;
    let info = CfgTestInfo::parse(content);
    assert!(info.test_ranges.is_empty());
}

#[test]
fn item_kind_pub_fn() {
    let content = r#"
#[cfg(test)]
pub fn test_helper() {
}
"#;
    let info = CfgTestInfo::parse(content);
    assert!(info.test_ranges.is_empty());
}

#[test]
fn item_kind_async_fn() {
    let content = r#"
#[cfg(test)]
async fn async_helper() {
}
"#;
    let info = CfgTestInfo::parse(content);
    assert!(info.test_ranges.is_empty());
}

#[test]
fn item_kind_unsafe_fn() {
    let content = r#"
#[cfg(test)]
unsafe fn unsafe_helper() {
}
"#;
    let info = CfgTestInfo::parse(content);
    assert!(info.test_ranges.is_empty());
}

#[test]
fn item_kind_pub_async_unsafe_fn() {
    let content = r#"
#[cfg(test)]
pub async unsafe fn complex_fn() {
}
"#;
    let info = CfgTestInfo::parse(content);
    assert!(info.test_ranges.is_empty());
}

#[test]
fn item_kind_const_fn() {
    let content = r#"
#[cfg(test)]
const fn const_helper() -> i32 {
    42
}
"#;
    let info = CfgTestInfo::parse(content);
    assert!(info.test_ranges.is_empty());
}

#[test]
fn item_kind_extern_fn() {
    let content = r#"
#[cfg(test)]
extern "C" fn extern_helper() {
}
"#;
    let info = CfgTestInfo::parse(content);
    assert!(info.test_ranges.is_empty());
}

#[test]
fn item_kind_impl() {
    let content = r#"
#[cfg(test)]
impl TestStruct {
    fn helper(&self) {}
}
"#;
    let info = CfgTestInfo::parse(content);
    assert!(info.test_ranges.is_empty());
}

#[test]
fn item_kind_impl_with_generics() {
    let content = r#"
#[cfg(test)]
impl<T> TestStruct<T> {
    fn helper(&self) {}
}
"#;
    let info = CfgTestInfo::parse(content);
    assert!(info.test_ranges.is_empty());
}

#[test]
fn item_kind_struct() {
    let content = r#"
#[cfg(test)]
struct TestFixture {
    value: i32,
}
"#;
    let info = CfgTestInfo::parse(content);
    assert!(info.test_ranges.is_empty());
}

#[test]
fn item_kind_pub_crate_struct() {
    let content = r#"
#[cfg(test)]
pub(crate) struct TestFixture {
    value: i32,
}
"#;
    let info = CfgTestInfo::parse(content);
    assert!(info.test_ranges.is_empty());
}

#[test]
fn item_kind_enum() {
    let content = r#"
#[cfg(test)]
enum TestEnum {
    A,
    B,
}
"#;
    let info = CfgTestInfo::parse(content);
    assert!(info.test_ranges.is_empty());
}

#[test]
fn item_kind_trait() {
    let content = r#"
#[cfg(test)]
trait TestTrait {
    fn method(&self);
}
"#;
    let info = CfgTestInfo::parse(content);
    assert!(info.test_ranges.is_empty());
}

#[test]
fn item_kind_multiple_blocks() {
    let content = r#"
fn source() {}

#[cfg(test)]
mod tests {
    fn test() {}
}

#[cfg(test)]
fn helper() {
}

#[cfg(test)]
struct Fixture {
    x: i32,
}
"#;
    let info = CfgTestInfo::parse(content);
    assert_eq!(info.test_ranges, vec![3..7]);
}

#[test]
fn item_kind_with_path_attr() {
    // Extra attributes between #[cfg(test)] and the item should be skipped
    let content = r#"
#[cfg(test)]
#[path = "tests.rs"]
mod tests {
    fn test() {}
}
"#;
    let info = CfgTestInfo::parse(content);
    assert_eq!(info.test_ranges.len(), 1);
}

#[test]
fn item_kind_pub_super() {
    let content = r#"
#[cfg(test)]
pub(super) fn test_helper() {
}
"#;
    let info = CfgTestInfo::parse(content);
    assert!(info.test_ranges.is_empty());
}

#[test]
fn item_kind_pub_in_path() {
    let content = r#"
#[cfg(test)]
pub(in crate::module) fn test_helper() {
}
"#;
    let info = CfgTestInfo::parse(content);
    assert!(info.test_ranges.is_empty());
}

#[test]
fn item_kind_macro_invocation() {
    let content = r#"
#[cfg(test)]
thread_local! {
    static MOCK: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}
"#;
    let info = CfgTestInfo::parse(content);
    assert!(info.test_ranges.is_empty());
}

#[test]
fn item_kind_lazy_static_macro() {
    let content = r#"
#[cfg(test)]
lazy_static! {
    static ref DB: Mutex<MockDb> = Mutex::new(MockDb::new());
}
"#;
    let info = CfgTestInfo::parse(content);
    assert!(info.test_ranges.is_empty());
}

#[test]
fn non_mod_items_not_in_test_ranges() {
    // Only `mod` blocks contribute to test_ranges; other item kinds
    // (helpers, fixtures, macros) are test infrastructure, not tests.
    let content = r#"
fn source() {}

#[cfg(test)]
mod tests {
    fn test() {}
}

#[cfg(test)]
fn helper() {
}

#[cfg(test)]
thread_local! {
    static MOCK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
struct Fixture {
    x: i32,
}
"#;
    let info = CfgTestInfo::parse(content);
    assert_eq!(info.test_ranges.len(), 1);
    assert!(info.is_test_line(3)); // #[cfg(test)] mod tests
    assert!(!info.is_test_line(9)); // #[cfg(test)] fn helper - NOT test
    assert!(!info.is_test_line(13)); // #[cfg(test)] thread_local! - NOT test
    assert!(!info.is_test_line(18)); // #[cfg(test)] struct Fixture - NOT test
}

#[test]
fn test_ranges_run_from_the_attribute_to_the_closing_brace() {
    let content = "fn a() {}\n\n#[cfg(test)]\nmod tests {\n\n    fn t() {}\n}\n";
    assert_eq!(test_ranges(content.as_bytes()), vec![2..7]);
}

#[test]
fn test_ranges_find_attributes_split_by_block_comments() {
    let content = "#[c/* */fg(test)]\nmod tests {\n    fn t() {}\n}\n";
    assert_eq!(test_ranges(content.as_bytes()), vec![0..4]);
}

#[test]
fn test_ranges_start_at_the_first_line_of_a_multiline_attribute() {
    let content = "fn a() {}\n#[cfg(\n\n    test\n)]\nmod tests {\n}\n";
    assert_eq!(test_ranges(content.as_bytes()), vec![1..7]);
}

#[test]
fn char_literals_are_found_by_character() {
    let content = "#[cfg(test)]\nmod tests {\n    const C: [char; 2] = ['é', '}'];\n}\nfn a() {}\n";
    assert_eq!(CfgTestInfo::parse(content).test_ranges, vec![0..4]);
}

#[test]
fn counts_files_that_are_not_utf8() {
    let content = b"#[cfg(test)]\nmod tests {\n    // \xff\n}\n";
    assert_eq!(test_ranges(content), vec![0..4]);
}

#[test]
fn comment_openers_in_strings_do_not_hide_test_modules() {
    let content = "fn source() {\n    let r = r#\"a \" /* b\"#;\n    let s = \"/* c\";\n    let u = \"http://x\"; {\n    }\n}\n\n#[cfg(test)]\nmod tests {\n    fn t() {}\n}\n";
    let info = CfgTestInfo::parse(content);
    assert_eq!(info.test_ranges, vec![7..11]);
}

#[test]
fn multiline_strings_with_braces_do_not_hide_test_modules() {
    let content = "fn source() {\n    let s = \"{\n  // }\n    }\";\n    let r = r#\"\n}\" {\n\"#;\n}\n\n#[cfg(test)]\nmod tests {\n    fn t() {}\n}\n";
    assert_eq!(CfgTestInfo::parse(content).test_ranges, vec![9..13]);
}

#[test]
fn multiline_strings_do_not_end_test_modules_early() {
    let content = "#[cfg(test)]\nmod tests {\n    const S: &str = \"\n}\n/* \\\"\n}\";\n    const R: &str = r##\"\n}\"#\n\"##;\n}\nfn a() {}\n";
    assert_eq!(CfgTestInfo::parse(content).test_ranges, vec![0..10]);
}

#[test]
fn attributes_inside_multiline_strings_are_not_detected() {
    let content = "const DOC: &str = \"\n#[cfg(test)]\nmod tests {\n}\n\";\nfn a() {}\n";
    assert!(CfgTestInfo::parse(content).test_ranges.is_empty());
    let content = "const DOC: &str = r#\"\n#[cfg(test)]\nmod tests {\n\"#;\nfn a() {}\n";
    assert!(CfgTestInfo::parse(content).test_ranges.is_empty());
}

#[test]
fn quotes_in_comments_and_characters_do_not_open_strings() {
    let content = "// don't \"\nfn a() { let c = '\"'; } /* \" */\n\n#[cfg(test)]\nmod tests {\n    fn t<'a>() {} // \"\n}\n";
    assert_eq!(CfgTestInfo::parse(content).test_ranges, vec![3..7]);
}

#[test]
fn escaped_backslash_ends_a_string_before_a_brace() {
    let content = "#[cfg(test)]\nmod tests {\n    const S: &str = \"\\\\\"; }\nfn a() {}\n";
    assert_eq!(CfgTestInfo::parse(content).test_ranges, vec![0..3]);
}

#[test]
fn escaped_character_literals_do_not_open_strings() {
    let content = "fn f(c: char) -> bool { matches!(c, '\\u{7b}'|'\"') }\n\n#[cfg(test)]\nmod tests {\n    fn t() {}\n}\n";
    assert_eq!(CfgTestInfo::parse(content).test_ranges, vec![2..6]);
}

#[test]
fn attributes_inside_strings_on_one_line_are_not_detected() {
    let content = "let s = \"#[cfg(test)]\";\nmod helpers {\n    fn h() {}\n}\n";
    assert!(CfgTestInfo::parse(content).test_ranges.is_empty());
}

#[test]
fn nested_block_comments_hide_test_modules() {
    let content = "/* a /* b */ \"\n#[cfg(test)]\nmod tests {\n}\n*/\nfn f() {}\n";
    assert!(CfgTestInfo::parse(content).test_ranges.is_empty());
}

#[yare::parameterized(
    strings_with_braces = { "fn a() {}\n\n#[cfg(test)]\nmod tests {\n    const S: &str = \"}\n// {\";\n    const C: char = '}';\n    const R: &str = r#\"}\"#;\n    /* } */\n}\n\nfn b() {}\n" },
    crlf = { "fn a() {}\r\n\r\n#[cfg(test)]\r\nmod tests {\r\n    // a comment\r\n\r\n    fn t() {}\r\n}\r\n\r\nfn b() {}\r\n" },
    unterminated = { "fn a() {}\n#[cfg(test)]\nmod tests {\n    fn t() {\n    }\n" },
    without_a_final_newline = { "fn a() {}\n#[cfg(test)]\nmod tests {\n    fn t() {}\n}" },
    blank_lines_after_the_module = { "#[cfg(test)]\nmod tests {\n}\n\n\n\n// after\nfn a() {}\n\n" },
    adjacent_modules = { "#[cfg(test)]\nmod a {\n}\n#[cfg(test)]\nmod b {\n    // b\n}\n" },
    nested_modules = { "#[cfg(test)]\nmod tests {\n    mod inner {\n        fn t() {}\n    }\n\n    /* a\n    } */\n}\nfn a() {}\n" },
    multiline_attribute = { "fn a() {}\n#[cfg(\n    test\n)]\n// between\nmod tests {\n}\n" },
    declared_without_a_body = { "#[cfg(test)]\nmod tests;\n#[cfg(test)]\nmod inline {\n}\n" },
    items_that_are_not_modules = { "#[cfg(test)]\nfn helper() {\n}\n#[cfg(test)]\nmod tests {\n}\n" },
    byte_order_mark = { "\u{feff}#[cfg(test)]\nmod tests {\n    fn t() {}\n}\nfn a() {}\n" },
    starting_inside_a_comment = { "/* a\n*/ #[cfg(test)]\nmod tests {\n    fn t() {}\n}\nfn a() {}\n" },
    starting_inside_a_string = { "const S: &str = \"a\n\"; #[cfg(test)]\nmod tests {\n    const T: &str = \"b\";\n}\nfn a() {}\n" },
    starting_after_code = { "#[derive(Debug)]\nstruct S; #[cfg(test)]\nmod tests {\n}\n" },
    attribute_in_a_doc_comment = { "/// #[cfg(test)]\n/// mod tests {\n/// }\nfn a() {}\n#[cfg(test)]\nmod tests {\n}\n" },
    without_modules = { "fn a() {}\n// cfg(test)\n" },
    split_after_the_hash = { "let a = \"#\"; #/* a */[cfg(test)]\nmod tests {\n}\n" },
    split_twice = { "#[c/* a */f/* b *//* c */g(test)]\nmod tests {\n}\n" },
    split_by_a_string = { "#[cf\"\"g(test)]\nmod tests {\n}\n" },
    split_by_a_raw_string = { "let s = r#\"a\"#[cfg(test)]\nmod tests {\n}\n" },
    split_by_a_string_with_a_comment = { "#[cf\"\"g(test)]\nmod tests {\n    /* a */\n}\n" },
    followed_by_code = { "#[cfg(test)]\nmod tests {\n}\nfn a() {}\n" },
    followed_by_a_string = { "#[cfg(test)]\nmod tests {\n}\nconst S: &str = \"a\";\n" },
    ending_inside_a_comment = { "#[cfg(test)]\nmod tests {\n} /* a\n*/\nfn a() {}\n" },
    block_comments_on_plain_lines = { "#[cfg(unix)]\nfn a() {}\n/* a\n#[cfg(test)]\nmod hidden {\n}\n*/\n#[cfg(test)]\nmod tests {\n}\n" },
)]
fn skipping_to_code_starts_counts_as_lexing_every_line(content: &str) {
    assert_eq!(count(content.as_bytes()), counted_alone(content));
    CfgTestInfo::parse(content);
}

#[test]
fn test_counts_hold_the_lines_of_test_modules() {
    let content =
        "fn a() {}\n\n#[cfg(test)]\nmod tests {\n    // \"{\n\n    fn t() {}\n}\n\nfn b() {}\n";
    let (counts, test) = count(content.as_bytes());
    assert_eq!(counts, LineCounts { blank: 3, comment: 1, code: 6 });
    assert_eq!(test, LineCounts { blank: 1, comment: 1, code: 4 });
}

#[test]
fn unterminated_test_modules_are_not_tests() {
    let content = "fn a() {}\n#[cfg(test)]\nmod tests {\n    fn t() {\n    }\n";
    assert_eq!(count(content.as_bytes()).1, LineCounts::default());
}

#[test]
fn crlf_test_modules_have_the_ranges_of_lf_ones() {
    let content = "fn a() {}\r\n\r\n#[cfg(test)]\r\nmod tests {\r\n    fn t() {}\r\n}\r\n";
    assert_eq!(CfgTestInfo::parse(content).test_ranges, vec![2..6]);
}
