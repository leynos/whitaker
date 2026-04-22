//! File-backed UI helper module ensuring the nested test remains permitted.

#[test]
fn check_cfg_test_detection_remains_scoped_to_the_test_module() {
    let option = Some("ok");
    option.expect("file-backed cfg(test) tests should still permit expect");
}
