## Hinweise zur Modullänge.

module_max_lines = { $lines ->
        [one] Das Modul { $module } umfasst { $lines } Zeile und überschreitet den zulässigen Grenzwert von { $limit }.
       *[other] Das Modul { $module } umfasst { $lines } Zeilen und überschreitet den zulässigen Grenzwert von { $limit }.
    }
    .note = Große Module sind schwerer zu überblicken und zu prüfen.
    .help = Teilen Sie { $module } in kleinere Module auf oder verringern Sie dessen Zuständigkeiten.
