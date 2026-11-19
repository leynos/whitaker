## パニックする unwrap_or_else フォールバックを抑制します。

no_unwrap_or_else_panic = { $receiver } に対する `unwrap_or_else` を、パニックしないフォールバックに置き換えてください。
    .note = `unwrap_or_else` に渡されたクロージャがパニックを引き起こします。
    .help = パニックする代わりに、エラーを伝播するか、説明的なメッセージを付けて `expect` を使用してください。
