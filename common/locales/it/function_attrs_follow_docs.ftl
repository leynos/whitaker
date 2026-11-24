## Diagnostica del lint sull’ordine degli attributi di funzione.

function_attrs_follow_docs = I commenti di documentazione su «{ $subject }» devono precedere gli altri attributi esterni.
    .note = L’attributo esterno { $attribute } compare prima del commento di documentazione.
    .help = Spostare il commento di documentazione in modo che compaia prima di { $attribute } sull’elemento.
