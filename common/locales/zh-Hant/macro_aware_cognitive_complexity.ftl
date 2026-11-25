## 手寫程式碼的認知複雜度應維持在設定的上限之內。

# `name`, `score`, and `limit` are rendered by the lint.
macro_aware_cognitive_complexity = `{ $name }` 的認知複雜度為 { $score }（上限為 { $limit }）。
    .note = 巨集展開所產生的分支不列入計算。
    .help = 請將該函式拆分為名稱清楚的較小函式。
