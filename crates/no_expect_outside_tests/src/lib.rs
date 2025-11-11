#![cfg_attr(feature = "dylint-driver", feature(rustc_private))]

#[cfg(feature = "dylint-driver")]
mod context;
#[cfg(feature = "dylint-driver")]
mod diagnostics;
#[cfg(feature = "dylint-driver")]
mod driver;
#[cfg(all(feature = "dylint-driver", test))]
mod behaviour;
#[cfg(all(feature = "dylint-driver", test))]
mod tests;
#[cfg(all(feature = "dylint-driver", test))]
mod ui {
    whitaker::declare_ui_tests!("ui");
}

#[cfg(feature = "dylint-driver")]
pub use driver::*;

#[cfg(not(feature = "dylint-driver"))]
mod stub {
    #[allow(dead_code)]
    pub fn no_expect_outside_tests_disabled_stub() {}
}
