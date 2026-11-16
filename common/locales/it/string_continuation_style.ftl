string_continuation_style = Unire le righe fisiche di questa stringa non raw con `concat!()`.
    .note = Il ritorno a capo con escape rimuove l’interruzione di riga fisica e l’indentazione successiva, nascondendo il confine tra i frammenti all’interno di un unico letterale.
    .help = Suddividere la stringa non raw in argomenti di `concat!()` in corrispondenza di ogni giunzione tra righe del sorgente.
    .label = giunzione tra righe del sorgente
    .suggestion = sostituire con un’espressione `concat!()`
