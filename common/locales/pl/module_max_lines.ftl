## Wskazówki dotyczące długości modułu.

module_max_lines = { $lines ->
        [one] Moduł { $module } ma { $lines } wiersz, co przekracza dozwolony limit { $limit }.
        [few] Moduł { $module } ma { $lines } wiersze, co przekracza dozwolony limit { $limit }.
        [many] Moduł { $module } ma { $lines } wierszy, co przekracza dozwolony limit { $limit }.
       *[other] Moduł { $module } ma { $lines } wiersza, co przekracza dozwolony limit { $limit }.
    }
    .note = Duże moduły trudniej przeglądać i recenzować.
    .help = Podziel moduł { $module } na mniejsze moduły lub ogranicz jego zakres odpowiedzialności.
