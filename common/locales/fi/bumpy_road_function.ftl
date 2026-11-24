## ”Bumpy Road” -kompleksisuuden tunnistin.

bumpy_road_function = Useita sisäkkäisen ehtologiikan klustereita kohteessa `{ $name }`.
    .note = { $count ->
        [one] Havaittiin { $count } kompleksisuushuippu, joka ylittää kynnysarvon { $threshold }.
       *[other] Havaittiin { $count } kompleksisuushuippua, jotka ylittävät kynnysarvon { $threshold }.
    }
    .help = Erota korostetut alueet apufunktioiksi klusteroituneen kompleksisuuden vähentämiseksi.
    .label = { $lines ->
        [one] Kompleksisuushuippu { $index } kattaa { $lines } rivin.
       *[other] Kompleksisuushuippu { $index } kattaa { $lines } riviä.
    }
