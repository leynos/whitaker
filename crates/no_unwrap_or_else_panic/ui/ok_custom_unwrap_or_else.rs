//! UI test: custom `unwrap_or_else`-like method on non-Option/Result should not lint.

#![deny(no_unwrap_or_else_panic)]

struct Wrapper;

impl Wrapper {
    fn unwrap_or_else<T>(&self, f: impl FnOnce() -> T) -> T {
        f()
    }
}

fn main() {
    let wrapper = Wrapper;
    let _ = wrapper.unwrap_or_else(|| panic!("should be ignored"));
}
