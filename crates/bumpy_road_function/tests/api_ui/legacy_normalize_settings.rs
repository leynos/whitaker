//! Compile-fail contract for the removed legacy settings function name.

use bumpy_road_function::analysis::normalise_settings;

fn main() {
    let _ = normalise_settings(Default::default());
}
