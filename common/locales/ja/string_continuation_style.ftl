string_continuation_style = この非 raw 文字列の物理行を `concat!()` で連結してください。
    .note = エスケープされた改行は物理的な改行とそれに続くインデントを取り除き、断片の境界を 1 つのリテラルの中に隠します。
    .help = 非 raw 文字列を、ソース行の結合箇所ごとに `concat!()` の引数へ分割してください。
    .label = ソース行の結合箇所
    .suggestion = `concat!()` 式に置き換える
