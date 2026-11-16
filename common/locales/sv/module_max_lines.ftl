## Vägledning om modullängd.

module_max_lines = { $lines ->
        [one] Modulen { $module } omfattar { $lines } rad och överskrider den tillåtna gränsen på { $limit }.
       *[other] Modulen { $module } omfattar { $lines } rader och överskrider den tillåtna gränsen på { $limit }.
    }
    .note = Stora moduler är svårare att navigera i och granska.
    .help = Dela upp { $module } i mindre moduler eller minska dess ansvarsområden.
