## הגבלת פעולות std::fs לצורך אכיפת קלט/פלט מבוסס יכולות.

no_std_fs_operations = פעולת std::fs בשם `{ $operation }` עוקפת את מדיניות מערכת הקבצים מבוססת היכולות.
    .note = std::fs ניגש לספריית העבודה הסביבתית; במקומו יש לקבל ידיות `cap_std::fs::Dir` ונתיבי camino, כדי שהקוראים יבחרו את משטח היכולות.
    .help = יש להעביר פרמטרים מסוג `cap_std::fs::Dir` יחד עם `camino::Utf8Path`/`Utf8PathBuf` דרך ממשקי ה-API במקום לקרוא ל-std::fs ישירות.

no_std_fs_operations_unknown_keys = מתעלמים ממפתחות לא מוכרים { $keys } ב־[no_std_fs_operations] עבור no_std_fs_operations { $version } (מהדורה { $revision }).
    .note = שאר המפתחות בקטע זה עדיין חלים; למפתחות שאינם מזוהים אין השפעה.
    .help = יש לעדכן את חבילת Whitaker למהדורה שמכירה את המפתחות האלה, או להסיר אותם מ־dylint.toml.

no_std_fs_operations_discarded_section = התצורה [no_std_fs_operations] עבור no_std_fs_operations { $version } (מהדורה { $revision }) נזרקת: { $error }
    .note = לא חלות החרגות; ה־lint משתמש בהגדרות ברירת המחדל שלו.
    .help = יש לתקן את הערך הפסול ב־dylint.toml, או לעדכן את חבילת Whitaker כך שתכיר את המפתח.
