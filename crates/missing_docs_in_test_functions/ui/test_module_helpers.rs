//! A `#[cfg(test)]` module is a module, so its functions are module-level.
//!
//! The inline `cfg` is unevaluated before expansion, so the module is walked
//! whether or not the harness will keep it, and the frame it pushes is a
//! `Module` frame. Both the undocumented helper and the undocumented test are
//! therefore reported; the documented pair is the control.

#![allow(dead_code)]

#[cfg(test)]
mod tests {
    fn undocumented_helper() {}

    /// A documented test, which is not reported.
    fn documented_helper() {}

    #[test]
    fn undocumented_test() { undocumented_helper(); }
}

/// Entry point, documented so it is not reported.
fn main() {}