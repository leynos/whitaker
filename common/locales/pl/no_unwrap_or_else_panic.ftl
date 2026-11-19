## Odradzanie rozwiązań zastępczych w unwrap_or_else, które wywołują panikę.

no_unwrap_or_else_panic = Zastąp unwrap_or_else na wartości { $receiver } rozwiązaniem zastępczym, które nie wywołuje paniki.
    .note = Domknięcie przekazane do unwrap_or_else wywołuje panikę.
    .help = Przekaż błąd dalej albo użyj `expect` z opisowym komunikatem zamiast wywoływać panikę.
