#![warn(conditional_max_n_branches)]

macro_rules! identity { ($expr:expr) => { $expr } }

// @codescene(disable:"Complex Conditional") issue #504 fixture: the three-conjunct call-site condition is the lint input (2026-11-12)
fn authored_if_in_macro(a: bool, b: bool, c: bool) {
    identity!(if a && b && c { println!("three"); });
}

fn main() { authored_if_in_macro(true, true, true); }
