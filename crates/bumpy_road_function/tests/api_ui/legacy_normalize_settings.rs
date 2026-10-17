//! Compile-fail contract for the removed legacy settings function name.

use bumpy_road_function::analysis::normalise_settings;

/// Exercise the removed legacy settings name in the compile-fail fixture.
fn main() {
    let _ = normalise_settings(Default::default());
}
