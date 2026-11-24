## Détecteur de complexité « Bumpy Road ».

bumpy_road_function = Plusieurs groupes de logique conditionnelle imbriquée dans `{ $name }`.
    .note = { $count ->
        [one] { $count } bosse de complexité détectée au-dessus du seuil { $threshold }.
       *[other] { $count } bosses de complexité détectées au-dessus du seuil { $threshold }.
    }
    .help = Extrayez des fonctions auxiliaires à partir des régions mises en évidence afin de réduire la complexité regroupée.
    .label = { $lines ->
        [one] La bosse de complexité { $index } s’étend sur { $lines } ligne.
       *[other] La bosse de complexité { $index } s’étend sur { $lines } lignes.
    }
