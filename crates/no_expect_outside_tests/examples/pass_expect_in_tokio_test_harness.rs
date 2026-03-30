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
