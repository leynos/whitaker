//! Gaelic (`gd`) fixture: an authored `if` above the configured limit.

pub mod fixture {
    //! Function whose authored branch exceeds the configured limit.

    pub fn authored_if(flag: bool) { if flag {} }
}

fn main() {}