## „Bumpy Road” komplexitásdetektor.

bumpy_road_function = Több beágyazott feltételes logikai csoport található itt: `{ $name }`.
    .note = { $count ->
        [one] { $count } komplexitási bukkanó észlelhető a(z) { $threshold } küszöbérték felett.
       *[other] { $count } komplexitási bukkanó észlelhető a(z) { $threshold } küszöbérték felett.
    }
    .help = Emeljen ki segédfüggvényeket a kiemelt régiókból a csoportosuló komplexitás csökkentéséhez.
    .label = { $lines ->
        [one] A(z) { $index }. komplexitási bukkanó { $lines } sort fog át.
       *[other] A(z) { $index }. komplexitási bukkanó { $lines } sort fog át.
    }
