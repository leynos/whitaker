## 限制在测试上下文之外调用 expect。

no_expect_outside_tests = 请避免在仅用于测试的代码之外对 { $receiver } 调用 `expect`。
    .note = 该调用位于“{ $context }”之内，而它未被识别为测试。
    .help = { $handling ->
        [option] 请处理 { $receiver } 的 `None` 变体，或将代码移入测试。
        [result] 请处理 { $receiver } 的 `Err` 变体，或将代码移入测试。
       *[other] 请处理 { $receiver } 的错误路径，或将代码移入测试。
    }
