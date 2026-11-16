## Vältä paniikin aiheuttavia `unwrap_or_else`-varavaihtoehtoja.

no_unwrap_or_else_panic = Korvaa kohteen { $receiver } unwrap_or_else varavaihtoehdolla, joka ei aiheuta paniikkia.
    .note = Metodille unwrap_or_else annettu sulkeuma aiheuttaa paniikin.
    .help = Välitä virhe eteenpäin tai käytä `expect`-metodia kuvaavan viestin kanssa paniikin aiheuttamisen sijaan.
