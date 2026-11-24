## A függvényattribútumok sorrendjét ellenőrző lint diagnosztikája.

function_attrs_follow_docs = A(z) „{ $subject }” elemek dokumentációs megjegyzéseinek meg kell előzniük a többi külső attribútumot.
    .note = A(z) { $attribute } külső attribútum a dokumentációs megjegyzés előtt áll.
    .help = Helyezze át a dokumentációs megjegyzést úgy, hogy az elemen a(z) { $attribute } attribútum előtt álljon.
