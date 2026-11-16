## 不鼓勵會 panic 的 unwrap_or_else 後援。

no_unwrap_or_else_panic = 請將 { $receiver } 上的 `unwrap_or_else` 替換為不會 panic 的後援。
    .note = 傳給 `unwrap_or_else` 的閉包會觸發 panic。
    .help = 請傳播錯誤，或使用帶有描述性訊息的 `expect`，而不要 panic。
