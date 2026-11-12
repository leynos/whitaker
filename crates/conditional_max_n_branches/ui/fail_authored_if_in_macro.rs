#![warn(conditional_max_n_branches)]

macro_rules! identity { ($expr:expr) => { $expr } }

fn authored_if_in_macro(a: bool, b: bool, c: bool) {
    identity!(if a && b && c { println!("three"); });
}

fn main() { authored_if_in_macro(true, true, true); }
