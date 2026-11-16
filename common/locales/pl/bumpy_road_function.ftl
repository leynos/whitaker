## Detektor złożoności „Bumpy Road”.

bumpy_road_function = W `{ $name }` występuje wiele skupisk zagnieżdżonej logiki warunkowej.
    .note = { $count ->
        [one] Wykryto { $count } wybój złożoności powyżej progu { $threshold }.
        [few] Wykryto { $count } wyboje złożoności powyżej progu { $threshold }.
        [many] Wykryto { $count } wybojów złożoności powyżej progu { $threshold }.
       *[other] Wykryto { $count } wyboju złożoności powyżej progu { $threshold }.
    }
    .help = Wyodrębnij funkcje pomocnicze z wyróżnionych obszarów, aby zmniejszyć złożoność skupioną w tych miejscach.
    .label = { $lines ->
        [one] Wybój złożoności { $index } obejmuje { $lines } wiersz.
        [few] Wybój złożoności { $index } obejmuje { $lines } wiersze.
        [many] Wybój złożoności { $index } obejmuje { $lines } wierszy.
       *[other] Wybój złożoności { $index } obejmuje { $lines } wiersza.
    }
