//! File-backed UI helper module for `#[cfg(test)]` ancestry coverage.

/// A plain helper, deliberately without `#[test]`.
///
/// The previous version of this file used a `#[test]` function, so it passed
/// on the test attribute and proved nothing about the file-backed `cfg(test)`
/// ancestry this fixture exists to cover.
fn first_item(items: &[u8]) -> u8 {
    items
        .first()
        .copied()
        .expect("file-backed cfg(test) ancestry permits expect")
}

#[test]
fn check_cfg_test_detection_in_file_backed_module() {
    assert_eq!(first_item(&[1]), 1);
}
