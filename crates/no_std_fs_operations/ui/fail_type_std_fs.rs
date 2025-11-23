//! UI fixture demonstrating disallowed `std::fs` type aliases.
#![deny(no_std_fs_operations)]

type AmbientFile = std::fs::File;

pub struct Holder {
    inner: AmbientFile,
}

impl Holder {
    pub fn new(inner: AmbientFile) -> Self {
        Self { inner }
    }

    pub fn into_inner(self) -> AmbientFile {
        self.inner
    }
}

fn main() {}
