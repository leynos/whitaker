//! UI fixture demonstrating disallowed `std::fs` method calls via traits.
#![deny(no_std_fs_operations)]

use std::fs::File;
use std::io::Read;

fn main() {
    let mut file = File::open("./demo.txt").expect("demo file should open");
    let mut buf = [0_u8; 4];
    file
        .read(&mut buf)
        .expect("reading from the demo file should succeed");
}
