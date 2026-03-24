# hoi4-parser

用于解析与还原 HOI4 / Paradox 风格脚本的 Rust 库。  
当前已完成基础解析器、还原器、兼容转义策略与性能基准 API。

## 功能概览

- `parse(input)`：将文本解析为 AST 文档结构 `Document`
- `generate(document)`：从 AST 还原文本
- 重复键元信息追踪：`duplicate_index` / `duplicate_suffix`
- 嵌套 quoted-object 支持：使用 `nested_quoted` 元信息替代魔法字段
- 兼容转义处理：比较符、方括号、冒号
- 基准接口：`benchmark_round_trip(input, iterations)`

## 快速开始

```rust
use hoi4_parser::{parse, generate};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"country = { name = "China" name = "PRC" }"#;
    let doc = parse(source)?;
    let out = generate(&doc)?;
    println!("{out}");
    Ok(())
}
```

## 兼容导出键名（重复键）

```rust
use hoi4_parser::{parse, export_key, Value};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc = parse("name = A\nname = B")?;
    if let Value::Object(root) = doc.root() {
        let second = &root.entries()[1];
        assert_eq!(export_key(second, true), "name$$1");
    }
    Ok(())
}
```

## 性能基准示例

```rust
use hoi4_parser::benchmark_round_trip;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = r#"country = { name = "China" effect = "set_var = { key = \"x\" value = 1 }" }"#;
    let report = benchmark_round_trip(input, 1000)?;
    println!("iterations: {}", report.iterations);
    println!("total: {:?}", report.parse_generate_total);
    println!("avg: {:?}", report.avg_per_iteration);
    Ok(())
}
```

## 开发验证

```bash
cargo test
```

## 测试集（fixtures）

已在 `tests/fixtures` 中准备基础样例集，并由 `tests/fixture_suite.rs` 统一执行：

- `basic_assignment.txt`：基础键值与对象块
- `duplicate_keys.txt`：重复键元信息
- `nested_quoted.txt`：嵌套 quoted-object
- `comment_and_hash.txt`：注释与字符串内 `#`
- `operators.txt`：比较运算符表达式（如 `>=`、`<`）
- `bracket_and_colon.txt`：方括号与冒号兼容转义
- `scoped_duplicate.txt`：作用域内/外重复键隔离
- `quote_adhesion.txt`：引号粘连字符串场景
- `array_block.txt`：数组风格块（非 `key = value` 子项）

后续新增样例时，只需添加 fixture 文件并在 `fixture_suite.rs` 增加对应断言。
