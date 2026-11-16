## 模块需要以内部文档注释开头。

module_must_have_inner_docs = 模块 { $module } 必须以内部文档注释开头。
    .note = 该模块的第一项不是 `//!` 形式的注释。
    .help = 请在顶部添加内部文档注释，说明 { $module } 的用途。
