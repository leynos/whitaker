## Von `unwrap_or_else`-Fallbacks abraten, die eine Panic auslösen.

no_unwrap_or_else_panic = Ersetzen Sie unwrap_or_else auf { $receiver } durch einen Fallback, der keine Panic auslöst.
    .note = Die an unwrap_or_else übergebene Closure löst eine Panic aus.
    .help = Geben Sie den Fehler weiter oder verwenden Sie `expect` mit einer aussagekräftigen Meldung, statt eine Panic auszulösen.
