use whitaker_test_macros::allow_fixture_expansion_lints;

#[allow_fixture_expansion_lints(unexpected)]
fn fixture() {}

fn main() {
    fixture();
}
