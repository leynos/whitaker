## Discourage unwrap_or_else(|| panic!(...)).

no_unwrap_or_else_panic = Replace unwrap_or_else(|| panic!(..)) on { $receiver } with error handling.
    .note = The closure supplied to unwrap_or_else triggers a panic.
    .help = Propagate the error or use expect with a descriptive message instead of panicking.
