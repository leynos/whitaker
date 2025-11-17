#![warn(module_must_have_inner_docs)]

// Module file contains the inline module data for the pass scenario.
#[path = "pass_file_module/documented.module"]
mod documented;

fn use_file_module() {
    documented::touch();
}

fn main() {
    use_file_module();
}
