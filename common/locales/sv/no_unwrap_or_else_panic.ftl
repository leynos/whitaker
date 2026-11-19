## Avråd från `unwrap_or_else`-fallbackar som orsakar panic.

no_unwrap_or_else_panic = Ersätt unwrap_or_else på { $receiver } med en fallback som inte orsakar panic.
    .note = Den closure som skickas till unwrap_or_else orsakar en panic.
    .help = Skicka felet vidare eller använd `expect` med ett beskrivande meddelande i stället för att orsaka panic.
