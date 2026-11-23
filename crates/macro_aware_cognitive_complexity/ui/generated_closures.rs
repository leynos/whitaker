// edition:2024
//! Generated closures and `async` blocks are transparent to the lint.
//!
//! RFC 0010 section 7.5: the author wrote no closure, so the authored
//! contents of a macro-generated closure or `async` block are charged to the
//! enclosing function. The configured limit is zero, so every scored function
//! warns, and each comment records the hand-derived score.

pub mod fixture {
    //! Macro shapes that wrap an authored body in a generated closure.

    use std::future::Future;
    use std::pin::Pin;

    macro_rules! wrapped {
        ($body:block) => {
            (|| $body)()
        };
    }

    // 3 = baseline + the two authored `if`s inside the generated closure.
    pub fn closure_two_branches(flag: bool, other: bool) {
        wrapped!({
            if flag {}
            if other {}
        });
    }

    // 3 = baseline + the authored `for` + the authored `if` nested in it.
    // The score has no nesting weight, so nesting only has to survive entry.
    pub fn closure_nested(values: &[bool]) {
        wrapped!({
            for value in values {
                if *value {}
            }
        });
    }

    // 2 = baseline + the authored `if` that follows the closure; the closure
    // itself adds nothing.
    pub fn closure_then_branch(flag: bool) {
        wrapped!({});
        if flag {}
    }

    // 1: the generated closure holds no authored branch.
    pub fn closure_empty() { wrapped!({}); }

    // 1 for this function; the authored closure is its own unit and scores 2
    // below, so its `if` is not charged here.
    pub fn authored_closure_stays_a_unit(flag: bool) {
        let check = |value: bool| if value {};
        check(flag);
    }

    // 1 for this function; the authored closure inside the generated closure
    // is still its own unit and scores 2 below.
    pub fn authored_closure_in_generated_closure(flag: bool) {
        wrapped!({
            let check = |value: bool| if value {};
            check(flag);
        });
    }

    // 2 = baseline + the authored `if` inside an `async` block written in
    // source; no unit scores the block, so the function carries it.
    pub fn authored_async_block(flag: bool) -> impl Future<Output = ()> {
        async move { if flag {} }
    }

    macro_rules! boxed {
        ($body:block) => {
            Box::pin(async move $body)
        };
    }

    // 3 = baseline + the two authored `if`s inside the generated `async move`
    // block, the shape `#[async_trait]` gives a method body.
    pub fn trait_method_shape(flag: bool, other: bool) -> Pin<Box<dyn Future<Output = ()>>> {
        boxed!({
            if flag {}
            if other {}
        })
    }

    // 3: the same body written as a plain `async fn` scores the same.
    pub async fn plain_async_control(flag: bool, other: bool) {
        if flag {}
        if other {}
    }

    // 1 = 3 branches - 2 authored returns: the generated block yields a
    // `Result`, so the returns are credited in full, as in the plain control.
    pub fn trait_method_result(
        flag: bool,
        other: bool,
    ) -> Pin<Box<dyn Future<Output = Result<u8, ()>>>> {
        boxed!({
            if flag { return Err(()); }
            if other { return Err(()); }
            Ok(1)
        })
    }

    // 1: the same body written as a plain `async fn` scores the same.
    pub async fn plain_async_result_control(flag: bool, other: bool) -> Result<u8, ()> {
        if flag { return Err(()); }
        if other { return Err(()); }
        Ok(1)
    }

    // 1 = 3 branches - 2 authored returns: the returns belong to the inner
    // `async` block, which yields a `Result`, though the outer generated
    // closure yields a boxed future.
    pub fn nested_wrappers_result(
        flag: bool,
        other: bool,
    ) -> Pin<Box<dyn Future<Output = Result<u8, ()>>>> {
        wrapped!({
            boxed!({
                if flag { return Err(()); }
                if other { return Err(()); }
                Ok(1)
            })
        })
    }

    // 2 = baseline + the authored `if`: the lone `return` belongs to the body,
    // and a `Result` block elsewhere in the function does not change its
    // credit.
    pub fn unrelated_result_block(flag: bool) {
        if flag { return; }
        let _block = async { Ok::<(), ()>(()) };
    }

    macro_rules! instrumented {
        ($body:block) => {
            async move $body.await
        };
    }

    // 3 = baseline + the two authored `if`s inside the generated block; the
    // `.await` the macro adds is not charged. This is the shape
    // `#[tracing::instrument]` gives an `async fn`.
    pub async fn instrument_shape(flag: bool, other: bool) {
        instrumented!({
            if flag {}
            if other {}
        })
    }

    pub struct Guard<F: FnMut()>(F);

    impl<F: FnMut()> Drop for Guard<F> {
        fn drop(&mut self) { (self.0)() }
    }

    macro_rules! defer {
        ($body:block) => {
            let _guard = Guard(|| $body);
        };
    }

    // 3 = baseline + the two authored `if`s in the deferred closure, the shape
    // of `scopeguard::defer!`.
    pub fn defer_shape(flag: bool, other: bool) {
        defer!({
            if flag {}
            if other {}
        });
    }
}

fn main() {}
