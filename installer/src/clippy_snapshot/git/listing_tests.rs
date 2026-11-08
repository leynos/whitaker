//! Bundle head parsing keeps the whole listing visible to policy.
//!
//! A dropped line would make an incomplete listing look complete, so a
//! malformed line must reach the caller as an error rather than disappear.

use super::parse_bundle_heads;

#[test]
fn a_well_formed_listing_keeps_the_object_and_ref_apart() {
    let heads =
        parse_bundle_heads("aaaa refs/heads/main\nbbbb HEAD").expect("the listing is well formed");

    assert_eq!(heads[0].object, "aaaa");
    assert_eq!(heads[0].name, "refs/heads/main");
    assert_eq!(heads[1].object, "bbbb");
    assert_eq!(heads[1].name, "HEAD");
}

#[test]
fn an_empty_listing_names_no_heads() {
    assert_eq!(parse_bundle_heads(""), Ok(Vec::new()));
}

#[test]
fn a_malformed_line_after_a_valid_head_is_reported_not_dropped() {
    let error = parse_bundle_heads("aaaa refs/heads/main\ngarbage")
        .expect_err("a line without a ref names no head");

    assert!(error.contains("garbage"), "{error}");
}
