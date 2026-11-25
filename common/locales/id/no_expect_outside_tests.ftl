## Batasi pemanggilan expect di luar konteks pengujian.

no_expect_outside_tests = Hindari memanggil `expect` pada { $receiver } di luar kode khusus pengujian.
    .note = Pemanggilan berasal dari dalam “{ $context }” yang tidak dikenali sebagai pengujian.
    .help = { $handling ->
        [option] Tangani varian `None` dari { $receiver } atau pindahkan kode ke dalam pengujian.
        [result] Tangani varian `Err` dari { $receiver } atau pindahkan kode ke dalam pengujian.
       *[other] Tangani jalur galat untuk { $receiver } atau pindahkan kode ke dalam pengujian.
    }
