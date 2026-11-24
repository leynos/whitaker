## 限制 std::fs 操作，以落实基于能力的 I/O。

no_std_fs_operations = std::fs 操作 `{ $operation }` 绕过了基于能力的文件系统策略。
    .note = std::fs 会访问隐式的工作目录；请改为接收 `cap_std::fs::Dir` 句柄和 camino 路径，以便由调用方选择能力范围。
    .help = 请通过 API 传递 `cap_std::fs::Dir` 以及 `camino::Utf8Path`/`Utf8PathBuf` 参数，而不要直接调用 std::fs。
