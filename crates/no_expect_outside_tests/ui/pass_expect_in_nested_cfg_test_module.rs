//! Positive UI fixture: a helper two modules below the `#[cfg(test)]` gate.
//!
//! Reproduces the layout that made three repositories red: a `#[cfg(test)]`
//! module whose own submodule carries no attribute, with the helper inside the
//! submodule. Only the outermost module is gated, so the lint has to walk the
//! ancestry rather than inspect the nearest module.
#![deny(no_expect_outside_tests)]

#[cfg(test)]
mod tests {
    //! The gated module; only this level carries `#[cfg(test)]`.

    mod helpers {
        //! An ungated submodule, so the lint must walk the ancestry.

        pub(super) fn first_item(items: &[u8]) -> u8 {
            items
                .first()
                .copied()
                .expect("cfg(test) ancestry two levels up permits expect")
        }
    }

    #[test]
    fn uses_the_helper() {
        assert_eq!(helpers::first_item(&[1]), 1);
    }
}

fn main() {}
