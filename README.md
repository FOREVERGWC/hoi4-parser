# hoi4-parser

用于解析与还原 HOI4 / Paradox 风格脚本的 Rust 库与工具集。

## 功能概览

- `parse(&str)` / `parse_owned(String)`：解析为 `Document`
- `generate(&Document)`：从 AST 还原文本
- 重复键元信息：`duplicate_index` / `duplicate_suffix`
- 嵌套 quoted script 支持：`nested_quoted` 元信息
- 匿名对象语义：数组中的裸 `{ ... }` 解析为 `AnonymousObject`
- 容错解析：
  - 对象块在 EOF 处允许隐式补全右花括号
  - 根级多余 `}` 忽略
- 兼容转义处理：比较符、方括号、冒号
- 规范格式化：
  - 键值项中的 `Object` / `AnonymousObject` / `Array` 统一按块状输出
  - 标量值默认内联输出
  - 数值尾零会按当前规范格式化裁剪
- 基准 API：`benchmark_round_trip(input, iterations)`

## 快速开始

```rust
use hoi4_parser::{generate, parse};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"country = { name = "China" name = "PRC" }"#;
    let doc = parse(source)?;
    let out = generate(&doc)?;
    println!("{out}");
    Ok(())
}
```

## 重要 API

```rust
use hoi4_parser::{export_key, parse, parse_owned, Value};

fn demo() -> Result<(), Box<dyn std::error::Error>> {
    let source = String::from("name = A\nname = B");
    let doc = parse_owned(source)?; // 已有 String 时可避免额外拷贝

    if let Value::Object(root) = doc.root() {
        let second = &root.entries()[1];
        assert_eq!(export_key(second, true), "name$$1");
    }
    Ok(())
}
```

## 可执行工具

- `cargo run --bin hoi4-parser`
  - 批量解析并输出到 `output`（路径在 `src/main.rs` 常量中）
  - 默认并行线程数为 `16`
  - 可用环境变量覆盖：`HOI4_PARSER_THREADS`
- `cargo run --bin compare_outputs -- <left_dir> <right_dir>`
  - 对比两个目录下 `.txt` 文件文本差异
- `cargo run --bin benchmark_report <fixture_path> <iterations> <warmup> <rounds>`
  - 输出多轮基准统计（min/median/p95/max）

## 开发与回归验证

```bash
cargo test
cargo run --bin hoi4-parser
cargo run --bin compare_outputs -- "C:\Users\91658\Desktop\Projects\Rust\hoi4-parser\output-java" "C:\Users\91658\Desktop\Projects\Rust\hoi4-parser\output"
```

## 当前对齐状态

- 当前与 Java 输出对比：`2102` 个文件中 `2097` 一致，剩余 `5` 处已知差异。
- 当前主要优化方向已转向性能；格式化策略已切换为统一、规范输出，不再追求保留源文件原始排版。

## 测试集（fixtures）

`tests/fixtures` 样例由 `tests/fixture_suite.rs` 统一执行，覆盖：

- 基础键值与对象块
- 空对象 / 空数组块格式
- 匿名对象数组块
- 重复键与作用域隔离
- nested quoted script
- 注释 / 字符串内 `#`
- 运算符与兼容转义
- 数组块、引号粘连、Unicode 名称等

其中当前 fixture 语义特别约束了两点：

- 匿名对象数组项输出为真实匿名块 `{ ... }`，不再生成旧式 `# = { ... }`
- 多行条件块允许数值在格式化时裁剪尾零，例如 `1.0 -> 1`
