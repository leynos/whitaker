## 手写代码的认知复杂度应保持在配置的上限之内。

# `name`, `score`, and `limit` are rendered by the lint.
macro_aware_cognitive_complexity = `{ $name }` 的认知复杂度为 { $score }（上限为 { $limit }）。
    .note = 宏展开生成的分支不计入。
    .help = 请将该函数拆分为命名清晰的更小函数。
