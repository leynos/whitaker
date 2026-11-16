## Discourage panicking unwrap_or_else fallbacks.

no_unwrap_or_else_panic = Replace unwrap_or_else on { $receiver } with a non-panicking fallback.
    .note = The closure supplied to unwrap_or_else triggers a panic.
    .help = Propagate the error or use expect with a descriptive message instead of panicking.
