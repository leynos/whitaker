## Detektor složitosti „Bumpy Road“.

bumpy_road_function = V `{ $name }` je několik shluků vnořené podmíněné logiky.
    .note = { $count ->
        [one] Zjištěn { $count } hrbol složitosti nad prahem { $threshold }.
        [few] Zjištěny { $count } hrboly složitosti nad prahem { $threshold }.
        [many] Zjištěno { $count } hrbolu složitosti nad prahem { $threshold }.
       *[other] Zjištěno { $count } hrbolů složitosti nad prahem { $threshold }.
    }
    .help = Extrahujte ze zvýrazněných oblastí pomocné funkce, a snižte tak složitost soustředěnou ve shlucích.
    .label = { $lines ->
        [one] Hrbol složitosti { $index } zabírá { $lines } řádek.
        [few] Hrbol složitosti { $index } zabírá { $lines } řádky.
        [many] Hrbol složitosti { $index } zabírá { $lines } řádku.
       *[other] Hrbol složitosti { $index } zabírá { $lines } řádků.
    }
