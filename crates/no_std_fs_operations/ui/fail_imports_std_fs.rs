//! UI fixture demonstrating disallowed `std::fs` imports and calls.
#![deny(no_std_fs_operations)]

use std::fs::{self, File};

fn main() {
    let _ = fs::read_to_string("./demo.txt");
    let _ = File::open("./demo.txt");
}
