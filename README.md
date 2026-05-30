# hoi4-parser

用于解析与还原 HOI4 / Paradox 风格脚本的 Rust 库，默认以库能力为主，仅保留少量通用工具。

## 当前项目状态

- 语言与版本：Rust edition `2024`
- 库入口：`src/lib.rs`
- 工具入口（可选）：
  - `src/bin/extract_paradox.rs`（批量提取脚本/LOC 到 JSON）
- 测试状态（本地最新检查）：`cargo test` 全部通过
  - 单元测试：`72` 通过
  - fixture 集成测试：`34` 通过

## 核心能力

- 脚本解析与还原：
  - `parse(&str)` / `parse_owned(String)` -> `Document`
  - `generate(&Document)` -> 还原文本
- AST 语义保真：
  - 重复键元信息：`duplicate_index` / `duplicate_suffix`
  - 匿名对象：数组中的裸 `{ ... }` 保留为 `AnonymousObject`
  - nested quoted script 元信息：`nested_quoted`
- 容错与兼容：
  - EOF 缺失右花括号容错
  - 根级多余 `}` 容错
  - 运算符、方括号、冒号等兼容转义恢复
- 性能统计 API：
  - `benchmark_round_trip(input, iterations)` 返回 `BenchReport`
- LOC 解析：
  - `parse_loc(&str)` -> `LocFile`
  - 支持 inline token 解析（图标、变量、括号块等）

## 快速开始（库）

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

## 常用 API 示例

```rust
use hoi4_parser::{benchmark_round_trip, export_key, parse_owned, Value};

fn demo() -> Result<(), Box<dyn std::error::Error>> {
    let source = String::from("name = A\nname = B");
    let doc = parse_owned(source)?;

    if let Value::Object(root) = doc.root() {
        let second = &root.entries()[1];
        assert_eq!(export_key(second, true), "name$$1");
    }

    let report = benchmark_round_trip("a = { b = 1 }", 10)?;
    println!("avg = {:?}", report.avg_per_iteration);
    Ok(())
}
```

## 工具（可选）

### 批量提取 JSON：`extract_paradox`

```bash
cargo run --bin extract_paradox -- <input_path> <output_dir> [--mode script|loc] [--ext yml,txt]
```

- `--mode script`（默认）：解析脚本并导出 JSON
- `--mode loc`：解析本地化 `.yml` 并导出 JSON
- 支持文件或目录输入；目录模式下会递归扫描并镜像输出目录结构

示例：

```bash
cargo run --bin extract_paradox -- "C:\mods\my_mod" "C:\tmp\json" --mode script --ext txt,gfx
cargo run --bin extract_paradox -- "C:\mods\my_mod\localisation" "C:\tmp\loc-json" --mode loc --ext yml
```

## 开发与验证

```bash
cargo test
cargo run --bin extract_paradox -- <input_path> <output_dir> --mode script
```

## 测试覆盖说明

`tests/fixtures` 由 `tests/fixture_suite.rs` 统一执行，覆盖：

- 基础键值/对象、空块、数组块、匿名对象数组块
- 重复键与作用域隔离
- nested quoted script
- 注释与字符串内 `#`
- 比较运算符与兼容符号恢复
- 多行条件块、前缀链、引号粘连
- Unicode 标识符与混合引号名称列表

当前 fixture 约束示例：

- 匿名对象数组项输出为真实匿名块 `{ ... }`，不再生成 `# = { ... }`
- 数值在规范化输出中会裁剪尾零（例如 `1.0 -> 1`）
