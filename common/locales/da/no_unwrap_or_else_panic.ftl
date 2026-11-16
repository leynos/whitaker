## Fraråd `unwrap_or_else`-fallbacks, der udløser panic.

no_unwrap_or_else_panic = Erstat unwrap_or_else på { $receiver } med en fallback, der ikke udløser panic.
    .note = Den closure, der gives til unwrap_or_else, udløser en panic.
    .help = Send fejlen videre, eller brug `expect` med en beskrivende meddelelse i stedet for at udløse panic.
