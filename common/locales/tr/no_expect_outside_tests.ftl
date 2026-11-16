## Test bağlamları dışındaki `expect` çağrıları kısıtlanır.

no_expect_outside_tests = Yalnızca testlere özgü kodun dışında { $receiver } üzerinde `expect` çağırmaktan kaçının.
    .note = Çağrı, test olarak tanınmayan “{ $context }” içinden yapılıyor.
    .help = { $handling ->
        [option] { $receiver } için `None` varyantını işleyin veya kodu bir teste taşıyın.
        [result] { $receiver } için `Err` varyantını işleyin veya kodu bir teste taşıyın.
       *[other] { $receiver } için hata yolunu işleyin veya kodu bir teste taşıyın.
    }
