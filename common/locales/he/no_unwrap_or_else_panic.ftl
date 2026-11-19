## הימנעות מחלופות unwrap_or_else שגורמות ל-panic.

no_unwrap_or_else_panic = יש להחליף את unwrap_or_else על { $receiver } בחלופה שאינה גורמת ל-panic.
    .note = הסגור שהועבר ל-unwrap_or_else גורם ל-panic.
    .help = יש להעביר את השגיאה הלאה או להשתמש ב-`expect` עם הודעה תיאורית במקום לגרום ל-panic.
