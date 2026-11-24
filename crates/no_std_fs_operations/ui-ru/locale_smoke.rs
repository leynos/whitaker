//! UI fixture verifying localized diagnostics for `no_std_fs_operations`.
#![deny(no_std_fs_operations)]

fn main() {
    let _ = std::fs::read("./demo.txt");
}
