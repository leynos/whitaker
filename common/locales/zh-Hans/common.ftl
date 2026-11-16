## 各 lint crate 共用的通用诊断辅助项。

-brand-name-whitaker = Whitaker
-term-lint = lint
-term-module = 模块
-term-branch = 分支
-term-test-coverage = 测试覆盖率

common-lint-count = lint 统计：{ $lint }。
    .note = 此区域设置中提供了 { $lint } 的消息。
    .help = 请为每个 lint 标识添加翻译，以保持测试覆盖率完整。
    .fallback-note = 回退诊断信息默认使用英语。

# Shown in diagnostics to refer to the preceding attribute name.
common-attribute-fallback = 前一个属性
