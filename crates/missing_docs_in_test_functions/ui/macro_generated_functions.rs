//! Functions a macro generates are never seen, let alone reported.
//!
//! `macro_rules!` expansion has not run when this pass visits the crate, so the
//! body of `make_function!` is a token tree and its invocation is a `MacCall`
//! node. Neither is an item, so nothing here is a candidate. The plain function
//! below is the control: it proves the run reached this file at all.

#![expect(dead_code, reason = "UI test fixture; functions are analysed but never invoked")]

macro_rules! make_function {
    ($name:ident) => {
        fn $name() {}
    };
}

make_function!(generated);

fn plain_undocumented() {}

/// Entry point, documented so it is not reported.
fn main() {}