## Ograniczenie operacji std::fs w celu wymuszenia operacji wejścia-wyjścia opartych na zdolnościach.

no_std_fs_operations = Operacja std::fs `{ $operation }` omija politykę systemu plików opartą na zdolnościach.
    .note = Moduł std::fs korzysta z otaczającego katalogu roboczego; zamiast tego przyjmuj uchwyty `cap_std::fs::Dir` i ścieżki camino, aby to wywołujący wybierał zakres zdolności.
    .help = Przekazuj przez swoje API parametry `cap_std::fs::Dir` oraz `camino::Utf8Path`/`Utf8PathBuf`, zamiast wywoływać std::fs bezpośrednio.

no_std_fs_operations_unknown_keys = Pomijam nieznane klucze { $keys } w [no_std_fs_operations] dla no_std_fs_operations { $version } (rewizja { $revision }).
    .note = Pozostałe klucze w tej sekcji nadal obowiązują; nierozpoznane klucze nie mają żadnego efektu.
    .help = Zaktualizuj zestaw Whitaker do rewizji, która rozumie te klucze, albo usuń je z dylint.toml.

no_std_fs_operations_discarded_section = Odrzucam konfigurację [no_std_fs_operations] dla no_std_fs_operations { $version } (rewizja { $revision }): { $error }
    .note = Nie obowiązują żadne wyjątki; lint używa ustawień domyślnych.
    .help = Popraw nieprawidłową wartość w dylint.toml albo zaktualizuj zestaw Whitaker, aby rozumiał ten klucz.
