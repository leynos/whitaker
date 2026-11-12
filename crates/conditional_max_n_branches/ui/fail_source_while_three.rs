#![warn(conditional_max_n_branches)]

fn source_while(mut a: bool, b: bool, c: bool) {
    while a && b && c { a = false; }
}

fn main() { source_while(true, true, true); }
