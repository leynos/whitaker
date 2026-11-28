//! Every documented form passes, including forms only a real compilation can
//! represent faithfully.
//!
//! `#[doc = include_str!(...)]` is the interesting one: before expansion its
//! value is a macro call rather than a string, so the classification is by
//! attribute shape rather than by resolved value.

#![expect(dead_code, reason = "UI test fixture; functions are analysed but never invoked")]

/// A line doc comment.
fn line_comment() {}

/**
 * A block doc comment.
 */
fn block_comment() {}

#[doc = "A name-value doc attribute."]
fn name_value() {}

#[doc = include_str!("auxiliary/doc_text.txt")]
fn included_value() {}

#[cfg_attr(test, doc = "A conditional doc attribute.")]
fn conditional_doc() {}

// The value of a conditional `doc` attribute may be a macro call. A
// classification that resolved the value rather than its shape would report
// this documented function.
#[cfg_attr(test, doc = include_str!("auxiliary/doc_text.txt"))]
fn conditional_included_value() {}

// The configuration predicate can itself be a list. Its inner commas must not
// be mistaken for the separator that ends the predicate.
#[cfg_attr(all(test, not(target_os = "none")), doc = "A conditional doc attribute.")]
fn conditional_doc_with_list_predicate() {}

// `cfg_attr` nests, and Rust expands the inner attribute, so this function is
// documented. The scan must enter the inner payload rather than stop at the
// outer attribute list.
#[cfg_attr(test, cfg_attr(test, doc = "A nested conditional doc attribute."))]
fn nested_conditional_doc() {}

// The raw spelling names the same attribute, so it documents too.
#[cfg_attr(test, r#doc = "A raw-identifier doc attribute.")]
fn raw_identifier_doc() {}

// Each inner form carries no outer documentation, so the fixture proves the
// inner comment alone satisfies the gate rather than passing on the outer one.
fn inner_line_comment() {
    //! Documented from inside the body.
    let _ = 1;
}

fn inner_block_comment() {
    /*! Documented from inside the body. */
    let _ = 1;
}

/// Entry point, documented so it is not reported.
fn main() {}
