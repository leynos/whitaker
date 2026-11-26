## 限制 std::fs 操作，以落实基于能力的 I/O。

no_std_fs_operations = std::fs 操作 `{ $operation }` 绕过了基于能力的文件系统策略。
    .note = std::fs 会访问隐式的工作目录；请改为接收 `cap_std::fs::Dir` 句柄和 camino 路径，以便由调用方选择能力范围。
    .help = 请通过 API 传递 `cap_std::fs::Dir` 以及 `camino::Utf8Path`/`Utf8PathBuf` 参数，而不要直接调用 std::fs。

no_std_fs_operations_unknown_keys = 正在忽略 [no_std_fs_operations] 中 no_std_fs_operations { $version }（修订 { $revision }）的未知键 { $keys }。
    .note = 本节中的其他键仍然有效；无法识别的键不会产生任何效果。
    .help = 请将 Whitaker 套件更新到能够理解这些键的修订，或从 dylint.toml 中移除它们。

no_std_fs_operations_discarded_section = 正在丢弃 no_std_fs_operations { $version }（修订 { $revision }）的 [no_std_fs_operations] 配置：{ $error }
    .note = 不应用任何排除项；lint 正在使用其默认设置。
    .help = 请修正 dylint.toml 中的无效值，或更新 Whitaker 套件使其理解该键。
