## 条件表达式应保持简洁。

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = 请将“{ $name }”精简到不超过 { $limit } 个分支。
    .note = “{ $name }”当前包含 { $branches } 个分支。
    .help = 请提取辅助函数或简化“{ $name }”，以减少分支。
