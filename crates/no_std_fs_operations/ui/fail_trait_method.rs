#![deny(no_std_fs_operations)]

use std::fs::File;
use std::io::Read;

fn main() {
    let mut file = File::open("./demo.txt").unwrap();
    let mut buf = [0_u8; 4];
    file.read(&mut buf).unwrap();
}
