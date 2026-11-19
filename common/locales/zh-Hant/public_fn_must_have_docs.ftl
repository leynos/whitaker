## 公開函式必須包含文件。

public_fn_must_have_docs = 公開函式 { $function } 需要文件註解。
    .note = 該函式已匯出，下游 crate 可以看見。
    .help = 請加入文件註解，說明 { $function } 的行為。
