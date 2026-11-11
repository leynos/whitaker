//! UI test: file-backed modules without inner docs should fail.
#![warn(module_must_have_inner_docs)]

#[path = "fail_file_module_missing_doc/undocumented.module"]
mod undocumented;

fn main() {
    undocumented::touch();
}