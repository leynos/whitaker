## 限制 std::fs 操作，以落實以能力為基礎的 I/O。

no_std_fs_operations = std::fs 操作 `{ $operation }` 繞過了以能力為基礎的檔案系統原則。
    .note = std::fs 會存取隱含的工作目錄；請改為接收 `cap_std::fs::Dir` 控制代碼與 camino 路徑，讓呼叫端選擇能力範圍。
    .help = 請透過 API 傳遞 `cap_std::fs::Dir` 以及 `camino::Utf8Path`/`Utf8PathBuf` 參數，而不要直接呼叫 std::fs。
