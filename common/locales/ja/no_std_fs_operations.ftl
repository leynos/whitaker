## ケイパビリティに基づく I/O を徹底するため、std::fs の操作を制限します。

no_std_fs_operations = std::fs の操作 `{ $operation }` は、ケイパビリティに基づくファイルシステムポリシーを迂回します。
    .note = std::fs は暗黙の作業ディレクトリに触れるため、呼び出し側がケイパビリティの範囲を選べるよう、代わりに `cap_std::fs::Dir` ハンドルと camino のパスを受け取ってください。
    .help = std::fs を直接呼び出す代わりに、`cap_std::fs::Dir` と `camino::Utf8Path`/`Utf8PathBuf` の引数を API に渡してください。

no_std_fs_operations_unknown_keys = no_std_fs_operations { $version } (リビジョン { $revision }) の [no_std_fs_operations] にある不明なキー { $keys } を無視します。
    .note = このセクションの他のキーは引き続き有効です。認識されないキーは効果を持ちません。
    .help = これらのキーを理解するリビジョンに Whitaker スイートを更新するか、dylint.toml から削除してください。

no_std_fs_operations_discarded_section = no_std_fs_operations { $version } (リビジョン { $revision }) の [no_std_fs_operations] 設定を破棄します: { $error }
    .note = 除外は適用されません。lint は既定の設定を使用します。
    .help = dylint.toml の無効な値を修正するか、キーを理解するように Whitaker スイートを更新してください。
