## 不鼓励会 panic 的 unwrap_or_else 回退。

no_unwrap_or_else_panic = 请将 { $receiver } 上的 `unwrap_or_else` 替换为不会 panic 的回退。
    .note = 传给 `unwrap_or_else` 的闭包会触发 panic。
    .help = 请传播错误，或使用带有描述性消息的 `expect`，而不要 panic。
