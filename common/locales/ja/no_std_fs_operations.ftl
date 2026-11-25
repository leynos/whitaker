## ケイパビリティに基づく I/O を徹底するため、std::fs の操作を制限します。

no_std_fs_operations = std::fs の操作 `{ $operation }` は、ケイパビリティに基づくファイルシステムポリシーを迂回します。
    .note = std::fs は暗黙の作業ディレクトリに触れるため、呼び出し側がケイパビリティの範囲を選べるよう、代わりに `cap_std::fs::Dir` ハンドルと camino のパスを受け取ってください。
    .help = std::fs を直接呼び出す代わりに、`cap_std::fs::Dir` と `camino::Utf8Path`/`Utf8PathBuf` の引数を API に渡してください。
