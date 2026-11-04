#![deny(string_continuation_style)]

const RAW: &str = r"alpha \
    beta";

fn main() {
    assert!(RAW.contains('\\'));
    assert!(RAW.contains('\n'));
}
