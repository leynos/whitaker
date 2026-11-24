## Vejledning om modullængde.

module_max_lines = { $lines ->
        [one] Modulet { $module } fylder { $lines } linje og overskrider den tilladte grænse på { $limit }.
       *[other] Modulet { $module } fylder { $lines } linjer og overskrider den tilladte grænse på { $limit }.
    }
    .note = Store moduler er sværere at navigere i og gennemgå.
    .help = Opdel { $module } i mindre moduler, eller reducer dets ansvarsområder.
