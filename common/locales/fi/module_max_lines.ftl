## Moduulin pituutta koskeva ohjeistus.

module_max_lines = { $lines ->
        [one] Moduuli { $module } on { $lines } rivin pituinen ja ylittää sallitun rajan { $limit }.
       *[other] Moduuli { $module } on { $lines } rivin pituinen ja ylittää sallitun rajan { $limit }.
    }
    .note = Suuria moduuleja on vaikeampi selata ja katselmoida.
    .help = Jaa moduuli { $module } pienempiin moduuleihin tai vähennä sen vastuita.
