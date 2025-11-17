#![warn(module_must_have_inner_docs)]

mod missing {
    pub fn demo() {}
}

fn main() {
    missing::demo();
}
