## 限制在測試情境之外呼叫 expect。

no_expect_outside_tests = 請避免在僅供測試的程式碼之外對 { $receiver } 呼叫 `expect`。
    .note = 該呼叫位於「{ $context }」之內，而它未被辨識為測試。
    .help = { $handling ->
        [option] 請處理 { $receiver } 的 `None` 變體，或將程式碼移入測試。
        [result] 請處理 { $receiver } 的 `Err` 變體，或將程式碼移入測試。
       *[other] 請處理 { $receiver } 的錯誤路徑，或將程式碼移入測試。
    }
