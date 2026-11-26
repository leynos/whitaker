## Обмеження операцій std::fs для забезпечення введення-виведення на основі можливостей.

no_std_fs_operations = Операція std::fs `{ $operation }` оминає політику файлової системи, засновану на можливостях.
    .note = Модуль std::fs звертається до навколишнього робочого каталогу; натомість приймайте дескриптори `cap_std::fs::Dir` і шляхи camino, щоб набір можливостей обирав код, який викликає.
    .help = Передавайте через свої API параметри `cap_std::fs::Dir` і `camino::Utf8Path`/`Utf8PathBuf`, а не викликайте std::fs безпосередньо.

no_std_fs_operations_unknown_keys = Невідомі ключі { $keys } у [no_std_fs_operations] для no_std_fs_operations { $version } (редакція { $revision }) ігноруються.
    .note = Інші ключі в цьому розділі далі діють; нерозпізнані ключі не мають впливу.
    .help = Оновіть набір Whitaker до редакції, яка розуміє ці ключі, або вилучіть їх із dylint.toml.

no_std_fs_operations_discarded_section = Конфігурацію [no_std_fs_operations] для no_std_fs_operations { $version } (редакція { $revision }) відкинуто: { $error }
    .note = Винятки не застосовуються; lint використовує типові налаштування.
    .help = Виправте недійсне значення в dylint.toml або оновіть набір Whitaker, щоб він розумів цей ключ.
