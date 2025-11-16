//! UI test fixture demonstrating that pattern matching branches are exempt from
//! the conditional_max_n_branches lint.
#![deny(conditional_max_n_branches)]

fn maybe_value() -> Option<i32> {
    Some(5)
}

fn main() {
    if let Some(value) = maybe_value() {
        println!("value: {value}");
    }
}
