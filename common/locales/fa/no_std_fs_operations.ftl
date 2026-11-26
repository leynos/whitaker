## عملیات std::fs برای اعمال ورودی/خروجی مبتنی بر قابلیت محدود می‌شود.

no_std_fs_operations = عملیات std::fs با نام `{ $operation }` سیاست فایل‌سیستم مبتنی بر قابلیت را دور می‌زند.
    .note = std::fs به پوشهٔ کاری محیط دسترسی پیدا می‌کند؛ به‌جای آن هندل‌های `cap_std::fs::Dir` و مسیرهای camino را بپذیرید تا فراخواننده‌ها سطح قابلیت را انتخاب کنند.
    .help = به‌جای فراخوانی مستقیم std::fs، پارامترهای `cap_std::fs::Dir` و `camino::Utf8Path`/`Utf8PathBuf` را از طریق API‌های خود عبور دهید.

no_std_fs_operations_unknown_keys = نادیده گرفتن کلیدهای ناشناختهٔ { $keys } در [no_std_fs_operations] برای no_std_fs_operations { $version } (بازبینی { $revision }).
    .note = کلیدهای دیگر این بخش همچنان اعمال می‌شوند؛ کلیدهای ناشناخته هیچ اثری ندارند.
    .help = مجموعهٔ Whitaker را به بازبینی‌ای به‌روزرسانی کنید که این کلیدها را می‌شناسد، یا آن‌ها را از dylint.toml حذف کنید.

no_std_fs_operations_discarded_section = دور انداختن پیکربندی [no_std_fs_operations] برای no_std_fs_operations { $version } (بازبینی { $revision }): { $error }
    .note = هیچ استثنایی اعمال نمی‌شود؛ lint از تنظیمات پیش‌فرض خود استفاده می‌کند.
    .help = مقدار نامعتبر را در dylint.toml اصلاح کنید یا مجموعهٔ Whitaker را به‌روزرسانی کنید تا کلید را بفهمد.
