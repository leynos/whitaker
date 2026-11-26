## 限制 std::fs 操作，以落實以能力為基礎的 I/O。

no_std_fs_operations = std::fs 操作 `{ $operation }` 繞過了以能力為基礎的檔案系統原則。
    .note = std::fs 會存取隱含的工作目錄；請改為接收 `cap_std::fs::Dir` 控制代碼與 camino 路徑，讓呼叫端選擇能力範圍。
    .help = 請透過 API 傳遞 `cap_std::fs::Dir` 以及 `camino::Utf8Path`/`Utf8PathBuf` 參數，而不要直接呼叫 std::fs。

no_std_fs_operations_unknown_keys = 正在忽略 [no_std_fs_operations] 中 no_std_fs_operations { $version }（修訂 { $revision }）的未知鍵 { $keys }。
    .note = 本節中的其他鍵仍然有效；無法辨識的鍵不會產生任何效果。
    .help = 請將 Whitaker 套件更新到能夠理解這些鍵的修訂，或從 dylint.toml 中移除它們。

no_std_fs_operations_discarded_section = 正在捨棄 no_std_fs_operations { $version }（修訂 { $revision }）的 [no_std_fs_operations] 設定：{ $error }
    .note = 不套用任何排除項目；lint 正在使用其預設設定。
    .help = 請修正 dylint.toml 中的無效值，或更新 Whitaker 套件使其理解該鍵。
