//! File-backed UI helper module for `#[cfg(test)]` ancestry coverage.

#[test]
fn check_cfg_test_detection_in_file_backed_module() {
    let option = Some("ok");
    option.expect("file-backed cfg(test) module with test attribute permits expect");
}
