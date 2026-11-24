## I test devono evitare le sezioni di esempio nella documentazione.

test_must_not_have_example = Rimuovere le sezioni di esempio dalla documentazione del test { $test }.
    .note = La documentazione di { $test } contiene il seguente elemento: «{ $reason }».
    .help = Eliminare l’esempio oppure spostarlo in una documentazione autonoma rivolta agli utenti.
