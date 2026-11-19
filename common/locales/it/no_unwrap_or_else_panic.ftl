## Scoraggiare le alternative di ripiego unwrap_or_else che vanno in panic.

no_unwrap_or_else_panic = Sostituire `unwrap_or_else` su { $receiver } con un’alternativa di ripiego che non vada in panic.
    .note = La chiusura passata a `unwrap_or_else` provoca un panic.
    .help = Propagare l’errore oppure usare `expect` con un messaggio descrittivo invece di andare in panic.
