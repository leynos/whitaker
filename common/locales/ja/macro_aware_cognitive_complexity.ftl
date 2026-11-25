## 作成者が書いたコードの認知的複雑度は、設定された上限内に収めるべきです。

# `name`, `score`, and `limit` are rendered by the lint.
macro_aware_cognitive_complexity = `{ $name }` の認知的複雑度は { $score } です（上限 { $limit }）。
    .note = マクロ展開によって生成された分岐は数えません。
    .help = 関数を、わかりやすい名前の小さな関数に分割してください。
