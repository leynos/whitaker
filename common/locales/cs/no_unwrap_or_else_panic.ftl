## Zamezení záložním řešením unwrap_or_else, která vyvolávají paniku.

no_unwrap_or_else_panic = Nahraďte unwrap_or_else na hodnotě { $receiver } záložním řešením, které nevyvolá paniku.
    .note = Uzávěr předaný unwrap_or_else vyvolává paniku.
    .help = Chybu propagujte, nebo místo vyvolání paniky použijte `expect` s výstižnou zprávou.
