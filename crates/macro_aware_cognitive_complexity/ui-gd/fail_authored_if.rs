//! Gaelic (`gd`) fixture: an authored `if` above the configured limit.

pub mod fixture {
    //! Function whose authored branch exceeds the configured limit.

    /// Branch on an authored condition, one over the zero limit.
    pub fn authored_if(flag: bool) { if flag {} }
}

/// Provide an entry point whose baseline alone exceeds the zero limit.
fn main() {}