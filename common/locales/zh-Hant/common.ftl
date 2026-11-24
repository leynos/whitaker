## 各 lint crate 共用的通用診斷輔助項目。

-brand-name-whitaker = Whitaker
-term-lint = lint
-term-module = 模組
-term-branch = 分支
-term-test-coverage = 測試涵蓋率

common-lint-count = lint 統計：{ $lint }。
    .note = 此地區設定提供 { $lint } 的訊息。
    .help = 請為每個 lint 識別碼加入翻譯，以維持測試涵蓋率完整。
    .fallback-note = 後援診斷訊息預設使用英文。

# Shown in diagnostics to refer to the preceding attribute name.
common-attribute-fallback = 前一個屬性
