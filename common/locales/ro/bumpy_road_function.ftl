## Detector de complexitate „Bumpy Road”.

bumpy_road_function = Există mai multe grupuri de logică condițională imbricată în `{ $name }`.
    .note = { $count ->
        [one] A fost detectată { $count } denivelare de complexitate peste pragul { $threshold }.
        [few] Au fost detectate { $count } denivelări de complexitate peste pragul { $threshold }.
       *[other] Au fost detectate { $count } de denivelări de complexitate peste pragul { $threshold }.
    }
    .help = Extrageți funcții auxiliare din regiunile evidențiate pentru a reduce complexitatea grupată.
    .label = { $lines ->
        [one] Denivelarea de complexitate { $index } se întinde pe { $lines } linie.
        [few] Denivelarea de complexitate { $index } se întinde pe { $lines } linii.
       *[other] Denivelarea de complexitate { $index } se întinde pe { $lines } de linii.
    }
