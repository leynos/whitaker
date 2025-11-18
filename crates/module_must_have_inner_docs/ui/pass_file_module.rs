//! UI test: file-backed modules with inner docs should pass.
#![warn(module_must_have_inner_docs)]

// This file-backed module contains the pass scenario.
#[path = "pass_file_module/documented.module"]
mod documented;

fn use_file_module() {
    documented::touch();
}

fn main() {
    use_file_module();
}
