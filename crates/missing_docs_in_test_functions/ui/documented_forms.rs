//! Every documented form passes, including forms only a real compilation can
//! represent faithfully.
//!
//! `#[doc = include_str!(...)]` is the interesting one: before expansion its
//! value is a macro call rather than a string, so the classification is by
//! attribute shape rather than by resolved value.

#![allow(dead_code)]

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

/// An inner doc comment in the body counts as documentation.
fn inner_line_comment() {
    //! Documented from inside the body.
    let _ = 1;
}

/** An inner block doc comment in the body counts as documentation. */
fn inner_block_comment() {
    /*! Documented from inside the body. */
    let _ = 1;
}

/// Entry point, documented so it is not reported.
fn main() {}
