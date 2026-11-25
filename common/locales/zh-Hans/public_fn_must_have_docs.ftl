## 公有函数必须包含文档。

public_fn_must_have_docs = 公有函数 { $function } 需要文档注释。
    .note = 该函数已导出，对下游 crate 可见。
    .help = 请添加文档注释，说明 { $function } 的行为。
