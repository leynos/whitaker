## Hindari cadangan unwrap_or_else yang memicu panic.

no_unwrap_or_else_panic = Ganti `unwrap_or_else` pada { $receiver } dengan cadangan yang tidak memicu panic.
    .note = Closure yang diberikan ke `unwrap_or_else` memicu panic.
    .help = Teruskan galat atau gunakan `expect` dengan pesan yang deskriptif alih-alih memicu panic.
