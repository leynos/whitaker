string_continuation_style = 请使用 `concat!()` 连接此非原始字符串中的物理行。
    .note = 转义换行符会移除物理换行及其后的缩进，将片段边界隐藏在同一个字面量中。
    .help = 请在每个源代码行连接处将该非原始字符串拆分为 `concat!()` 的参数。
    .label = 源代码行连接处
    .suggestion = 替换为 `concat!()` 表达式
