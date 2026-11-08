//! Bundle head parsing keeps the whole listing visible to policy.
//!
//! A dropped line would make an incomplete listing look complete, so a
//! malformed line must reach the caller as an error rather than disappear.

use super::parse_bundle_heads;

#[test]
fn a_well_formed_listing_keeps_the_object_and_ref_apart() {
    let heads =
        parse_bundle_heads("aaaa refs/heads/main\nbbbb HEAD").expect("the listing is well formed");

    let found: Vec<(&str, &str)> = heads
        .iter()
        .map(|head| (head.object.as_str(), head.name.as_str()))
        .collect();
    assert_eq!(found, vec![("aaaa", "refs/heads/main"), ("bbbb", "HEAD")]);
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
