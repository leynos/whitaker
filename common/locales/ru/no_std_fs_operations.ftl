## Ограничение операций std::fs для обеспечения ввода-вывода на основе возможностей.

no_std_fs_operations = Операция std::fs `{ $operation }` обходит политику файловой системы, основанную на возможностях.
    .note = Модуль std::fs обращается к окружающему рабочему каталогу; вместо этого принимайте дескрипторы `cap_std::fs::Dir` и пути camino, чтобы набор возможностей выбирал вызывающий код.
    .help = Передавайте через свои API параметры `cap_std::fs::Dir` и `camino::Utf8Path`/`Utf8PathBuf`, а не вызывайте std::fs напрямую.

no_std_fs_operations_unknown_keys = Неизвестные ключи { $keys } в [no_std_fs_operations] для no_std_fs_operations { $version } (редакция { $revision }) игнорируются.
    .note = Остальные ключи в этом разделе по-прежнему действуют; нераспознанные ключи не оказывают влияния.
    .help = Обновите набор Whitaker до редакции, которая понимает эти ключи, или удалите их из dylint.toml.

no_std_fs_operations_discarded_section = Конфигурация [no_std_fs_operations] для no_std_fs_operations { $version } (редакция { $revision }) отбрасывается: { $error }
    .note = Исключения не применяются; lint использует настройки по умолчанию.
    .help = Исправьте недопустимое значение в dylint.toml или обновите набор Whitaker, чтобы он понимал этот ключ.
