## Ograniczenie operacji std::fs w celu wymuszenia operacji wejścia-wyjścia opartych na zdolnościach.

no_std_fs_operations = Operacja std::fs `{ $operation }` omija politykę systemu plików opartą na zdolnościach.
    .note = Moduł std::fs korzysta z otaczającego katalogu roboczego; zamiast tego przyjmuj uchwyty `cap_std::fs::Dir` i ścieżki camino, aby to wywołujący wybierał zakres zdolności.
    .help = Przekazuj przez swoje API parametry `cap_std::fs::Dir` oraz `camino::Utf8Path`/`Utf8PathBuf`, zamiast wywoływać std::fs bezpośrednio.
