## Canfodydd "Bumpy Road" ar gyfer cymhlethdod.

bumpy_road_function = Mae sawl clwstwr o resymeg amodol nythiedig yn `{ $name }`.
    .note = { $count ->
        [zero] Canfuwyd { $count } o "bumps" cymhlethdod uwchlaw’r trothwy { $threshold }.
        [one] Canfuwyd { $count } o "bump" cymhlethdod uwchlaw’r trothwy { $threshold }.
        [two] Canfuwyd { $count } o "bumps" cymhlethdod uwchlaw’r trothwy { $threshold }.
        [few] Canfuwyd { $count } o "bumps" cymhlethdod uwchlaw’r trothwy { $threshold }.
        [many] Canfuwyd { $count } o "bumps" cymhlethdod uwchlaw’r trothwy { $threshold }.
       *[other] Canfuwyd { $count } o "bumps" cymhlethdod uwchlaw’r trothwy { $threshold }.
    }
    .help = Tynnwch swyddogaethau cynorthwyol o’r rhanbarthau a amlygwyd i leihau’r cymhlethdod clwstredig.
    .label = { $lines ->
        [zero] Mae bump cymhlethdod { $index } yn ymestyn dros { $lines } o linellau.
        [one] Mae bump cymhlethdod { $index } yn ymestyn dros { $lines } llinell.
        [two] Mae bump cymhlethdod { $index } yn ymestyn dros { $lines } o linellau.
        [few] Mae bump cymhlethdod { $index } yn ymestyn dros { $lines } o linellau.
        [many] Mae bump cymhlethdod { $index } yn ymestyn dros { $lines } o linellau.
       *[other] Mae bump cymhlethdod { $index } yn ymestyn dros { $lines } o linellau.
    }
