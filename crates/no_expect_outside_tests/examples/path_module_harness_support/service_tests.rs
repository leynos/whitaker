//! Test module loaded via `#[path]` with a non-standard name.

#[tokio::test]
async fn expect_in_path_module_is_allowed() {
    let value: Result<&str, ()> = Ok("ok");
    value.expect("path-loaded module test should permit expect");
}
