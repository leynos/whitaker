## テストコンテキスト外での expect 呼び出しを制限します。

no_expect_outside_tests = テスト専用コードの外では、{ $receiver } に対して `expect` を呼び出さないでください。
    .note = この呼び出しは「{ $context }」の中にありますが、これはテストとして認識されていません。
    .help = { $handling ->
        [option] { $receiver } の `None` バリアントを処理するか、コードをテストに移動してください。
        [result] { $receiver } の `Err` バリアントを処理するか、コードをテストに移動してください。
       *[other] { $receiver } のエラー経路を処理するか、コードをテストに移動してください。
    }
