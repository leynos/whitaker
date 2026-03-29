#[tokio::test]
async fn pass_expect_in_tokio_test_harness() {
    assert_eq!(2 + 2, 4);
}

fn main() {}
