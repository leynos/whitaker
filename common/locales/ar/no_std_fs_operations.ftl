## تقييد عمليات std::fs لفرض الإدخال/الإخراج المبني على القدرات.

no_std_fs_operations = عملية std::fs المسماة `{ $operation }` تتجاوز سياسة نظام الملفات المبنية على القدرات.
    .note = يتعامل std::fs مع دليل العمل المحيط؛ اقبل بدلًا منه مقابض `cap_std::fs::Dir` ومسارات camino ليختار المستدعون سطح القدرات.
    .help = مرّر معاملات `cap_std::fs::Dir` مع `camino::Utf8Path`/`Utf8PathBuf` عبر واجهاتك البرمجية بدلًا من استدعاء std::fs مباشرة.

no_std_fs_operations_unknown_keys = تجاهل المفاتيح غير المعروفة { $keys } في [no_std_fs_operations] من أجل no_std_fs_operations { $version } (المراجعة { $revision }).
    .note = لا تزال المفاتيح الأخرى في هذا القسم سارية؛ ولا تأثير للمفاتيح غير المعروفة.
    .help = حدّث حزمة Whitaker إلى مراجعة تفهم هذه المفاتيح، أو أزلها من dylint.toml.

no_std_fs_operations_discarded_section = تجاهل تهيئة [no_std_fs_operations] من أجل no_std_fs_operations { $version } (المراجعة { $revision }): { $error }
    .note = لا تُطبَّق أي استثناءات؛ ويستخدم الـ lint إعداداته الافتراضية.
    .help = صحّح القيمة غير الصالحة في dylint.toml، أو حدّث حزمة Whitaker كي تفهم المفتاح.
