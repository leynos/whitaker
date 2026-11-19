## 条件式は浅く保つべきです。

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = 「{ $name }」の分岐を { $limit } 個以下にまとめてください。
    .note = 「{ $name }」には現在 { $branches } 個の分岐があります。
    .help = ヘルパー関数を抽出するか、「{ $name }」を単純化して分岐を減らしてください。
