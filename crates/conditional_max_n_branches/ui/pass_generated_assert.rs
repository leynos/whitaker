#![warn(conditional_max_n_branches)]

macro_rules! generated_if {
    ($a:expr, $b:expr, $c:expr) => {
        if $a && $b && $c { println!("three"); }
    };
}

fn generated_assert(a: bool, b: bool, c: bool) { assert!(a && b && c); }

fn generated_local_macro(a: bool, b: bool, c: bool) { generated_if!(a, b, c); }

fn main() { generated_assert(true, true, true); generated_local_macro(true, true, true); }
