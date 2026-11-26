//! UI test: one `cfg_attr` yielding a non-doc and a doc attribute is reported.
//!
//! Both attributes come from the same `cfg_attr`, so this fixture pins how the
//! adapter orders two records that share one source attribute. The injected
//! `warn(dead_code)` is written first and wins the ordering check, so the
//! module is reported even though the same attribute also supplies `doc`.
#![warn(module_must_have_inner_docs)]

mod documented {
    #![cfg_attr(all(), warn(dead_code), doc = "Documentation from the same cfg_attr.")]

    pub fn example() {}
}

fn main() {
    let _ = documented::example();
}