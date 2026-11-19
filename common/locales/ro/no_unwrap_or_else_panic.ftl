## Descurajarea soluțiilor de rezervă unwrap_or_else care intră în panică.

no_unwrap_or_else_panic = Înlocuiți `unwrap_or_else` pe { $receiver } cu o soluție de rezervă care nu intră în panică.
    .note = Închiderea furnizată către `unwrap_or_else` declanșează o panică.
    .help = Propagați eroarea sau folosiți `expect` cu un mesaj descriptiv, în loc să intrați în panică.
