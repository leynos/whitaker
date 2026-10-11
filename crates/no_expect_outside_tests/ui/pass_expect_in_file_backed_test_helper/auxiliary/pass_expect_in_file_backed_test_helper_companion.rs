//! File-backed companion module whose helpers call `.expect(...)`.

/// A plain helper, not a test and not a fixture.
fn parse_fixture_value() -> u32 {
    "7".parse::<u32>()
        .expect("a cfg(test) companion helper may expect")
}

/// A helper reached through a nested module of the companion.
mod nested {
    pub(super) fn first_item() -> &'static str {
        ["a"].first()
            .copied()
            .expect("a nested companion helper may expect")
    }
}

#[test]
fn helpers_are_used_by_a_test() {
    assert_eq!(parse_fixture_value(), 7);
    assert_eq!(nested::first_item(), "a");
}
