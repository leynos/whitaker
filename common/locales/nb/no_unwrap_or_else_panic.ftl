## Fraråd `unwrap_or_else`-fallbacker som utløser panic.

no_unwrap_or_else_panic = Erstatt unwrap_or_else på { $receiver } med en fallback som ikke utløser panic.
    .note = Closuren som gis til unwrap_or_else, utløser en panic.
    .help = Send feilen videre, eller bruk `expect` med en beskrivende melding i stedet for å utløse panic.
