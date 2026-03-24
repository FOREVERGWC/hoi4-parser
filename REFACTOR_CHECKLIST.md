# HOI4 Parser Rust Refactor Checklist

创建日期：2026-03-24  
项目路径：`hoi4-parser`  
目标：将 `ParadoxParserUtil.java` 的解析器与还原器能力重构为 Rust 版本，并保留兼容策略。

## 使用说明

- 每完成一个步骤，将对应项从 `[ ]` 改为 `[x]`。
- 如果某一步拆分成子任务，可在该项下追加子清单。
- 所有状态变更都记录在“执行日志”中，便于回溯。

## 实施检查清单

- [x] 0. 创建并初始化本追踪文档
- [x] 1. 定义公开 API（`parse`/`generate`）与统一错误类型
- [x] 2. 设计并实现 AST（支持对象、数组、标量、重复键元信息）
- [x] 3. 实现 tokenizer（字符串、转义、注释、花括号、等号等词法单元）
- [x] 4. 实现 parser（递归下降，支持 `key = value` 与 `key = { ... }`）
- [x] 5. 实现重复键兼容策略（内部保序；JSON 导出时可选 `$$HEX`）
- [x] 6. 实现嵌套 quoted-object 解析与回写策略（替代 `isNest` 魔法键）
- [x] 7. 实现 generator（AST 直出 Paradox 文本，避免 JSON 字符串替换链）
- [x] 8. 完成比较符、方括号、冒号等转义与反转义兼容规则
- [x] 9. 构建 round-trip 回归测试（解析后再还原，语义一致）
- [x] 10. 增加典型边界测试（注释、引号粘连、重复键、嵌套块）
- [x] 11. 进行性能基准与必要优化（解析吞吐、内存占用）
- [x] 12. 整理文档与对外使用示例

## 当前执行步骤

> 正在执行：`全部步骤已完成，等待你验收`

## 执行日志

- [2026-03-24] 步骤 0 完成：创建追踪文档并写入初始清单。
- [2026-03-24] 步骤 1 完成：实现 `parse`/`generate` 公开 API、统一错误类型 `Hoi4ParserError`，并通过 `cargo test`。
- [2026-03-24] 步骤 2 完成：新增 `ast` 模块并定义 `Value/ObjectNode/Entry/EntryMetadata`，`Document` 接入 AST 根节点；新增重复键元信息测试并通过 `cargo test`。
- [2026-03-24] 步骤 3 完成：新增 `tokenizer` 模块，支持字符串、转义、注释、花括号、等号与换行 token；补充词法单测并通过 `cargo test`。
- [2026-03-24] 步骤 4 完成：新增 `parser` 模块并接入 `parse()` 主流程，支持 `key = value` 和 `key = { ... }` 基础递归解析；补充语法单测并通过 `cargo test`。
- [2026-03-24] 步骤 5 完成：在同级作用域记录重复键元信息（`duplicate_index`/`duplicate_suffix`），新增兼容导出键函数（可选 `$$HEX`）并通过 `cargo test`。
- [2026-03-24] 步骤 6 完成：新增 `nested` 模块（quoted-object 编码/解码策略），在 parser 中自动识别并解析嵌套 quoted-object，使用 `metadata.nested_quoted` 标记，不再依赖 `isNest` 字段；通过 `cargo test`。
- [2026-03-24] 步骤 7 完成：新增 `generator` 模块并接管 `generate()`，从 AST 直接生成文本，修复了顶层额外花括号回归；通过 `cargo test`。
- [2026-03-24] 步骤 8 完成：新增兼容转义层（比较符、方括号、冒号）并接入 parser/generator 读写路径；新增兼容测试并通过 `cargo test`。
- [2026-03-24] 步骤 9 完成：新增 round-trip 语义回归测试（解析 -> 还原 -> 再解析，AST 语义一致），并修复 nested quoted 回写格式问题；通过 `cargo test`。
- [2026-03-24] 步骤 10 完成：补充典型边界测试（字符串内 `#`、未闭合引号、重复键作用域隔离等）；通过 `cargo test`。
- [2026-03-24] 步骤 11 完成：新增 `perf` 模块与 `benchmark_round_trip` 基准 API，输出总耗时/平均耗时/输入大小；通过 `cargo test`。
- [2026-03-24] 步骤 12 完成：新增 `README.md`，补齐功能说明、快速开始、重复键兼容示例、性能基准示例与开发验证命令。
- [2026-03-24] 附加任务完成：建立 `tests/fixtures` 测试集与 `tests/fixture_suite.rs` 集成测试执行器，覆盖基础赋值、重复键、嵌套 quoted-object；`cargo test` 通过（单元测试 + 集成测试）。
- [2026-03-24] 附加任务完成：扩展真实场景 fixture（注释/hash、比较符、方括号/冒号、作用域重复键），并修复对应解析/还原兼容问题（多 token 标量表达式、符号反转义）；`cargo test` 全通过。
- [2026-03-24] 附加任务完成：新增 Java 对照高风险样例（引号粘连、数组风格块），并修复 parser 歧义块回退逻辑（对象解析失败时回退数组解析）；`cargo test` 全通过。
