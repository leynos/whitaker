## 模組需要以內部文件註解開頭。

module_must_have_inner_docs = 模組 { $module } 必須以內部文件註解開頭。
    .note = 該模組的第一個項目不是 `//!` 形式的註解。
    .help = 請在頂端加入內部文件註解，說明 { $module } 的用途。
