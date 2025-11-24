//! UI test: macro-generated modules should be ignored by the lint.
#![warn(module_must_have_inner_docs)]

macro_rules! make_module {
    ($name:ident) => {
        mod $name {
            pub fn value() -> usize { 1 }
        }
    };
}

make_module!(generated);

fn main() {
    let _ = generated::value();
}
