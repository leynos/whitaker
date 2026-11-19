## Veiledning om modullengde.

module_max_lines = { $lines ->
        [one] Modulen { $module } går over { $lines } linje og overskrider den tillatte grensen på { $limit }.
       *[other] Modulen { $module } går over { $lines } linjer og overskrider den tillatte grensen på { $limit }.
    }
    .note = Store moduler er vanskeligere å navigere i og gjennomgå.
    .help = Del { $module } opp i mindre moduler, eller reduser ansvarsområdene til modulen.
