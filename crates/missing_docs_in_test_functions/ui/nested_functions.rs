//! Functions nested anywhere other than a module are out of scope.
//!
//! `missing_docs` ignores items inside bodies for the same reason: a nested
//! helper is an implementation detail of the item that contains it, not a
//! member of the module's interface. Each container shape that can hold a
//! function is represented, so a stack that forgets to push an `Other` frame
//! reports one of them.

#![allow(dead_code)]

/// A function whose body holds most of the nested shapes below.
fn outer() {
    fn inside_a_function() {}
    inside_a_function();

    const _: () = {
        fn inside_a_const_block() {}
    };

    let _ = {
        fn inside_a_block_expression() {}
        inside_a_block_expression()
    };

    struct Local;

    impl Local {
        fn inside_a_local_impl(&self) {}
    }

    let local = Local;
    local.inside_a_local_impl();
}

enum Choice {
    First = {
        const fn inside_an_enum_discriminant() -> isize { 0 }
        inside_an_enum_discriminant()
    },
    Second,
}

/// Entry point, documented so it is not reported.
fn main() {
    outer();
    let _ = Choice::First;
    let _ = Choice::Second;
}