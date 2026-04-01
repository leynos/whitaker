//! Regression example covering async `#[tokio::test]` handling for
//! `no_expect_outside_tests`.
//!
//! The nested `expect(...)` calls inside `pass_expect_in_tokio_test_harness`
//! validate that async test wrappers still count as test-only code even when
//! the lint inspects closure and async-block bodies. `main` remains empty
//! because the test function itself is the validation point.

#[tokio::test]
async fn pass_expect_in_tokio_test_harness() {
    let nested_value = || Some(4).expect("nested closure should inherit async test context");
    let computed = async move {
        Ok::<_, ()>(nested_value()).expect("nested async block should inherit async test context")
    }
    .await;

    assert_eq!(computed, 4);
}

fn main() {}
