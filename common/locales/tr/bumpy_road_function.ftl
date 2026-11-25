## "Bumpy Road" karmaşıklık algılayıcısı.

bumpy_road_function = `{ $name }` içinde iç içe koşullu mantıktan oluşan birden çok küme var.
    .note = { $count ->
        [one] { $threshold } eşiğinin üzerinde { $count } karmaşıklık tümseği algılandı.
       *[other] { $threshold } eşiğinin üzerinde { $count } karmaşıklık tümseği algılandı.
    }
    .help = Kümelenmiş karmaşıklığı azaltmak için vurgulanan bölgelerden yardımcı işlevler ayıklayın.
    .label = { $lines ->
        [one] Karmaşıklık tümseği { $index }, { $lines } satır kaplıyor.
       *[other] Karmaşıklık tümseği { $index }, { $lines } satır kaplıyor.
    }
