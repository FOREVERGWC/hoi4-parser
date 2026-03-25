mod ast;
mod compat;
mod error;
mod generator;
mod nested;
mod parser;
mod perf;
mod tokenizer;

pub use ast::{Entry, EntryMetadata, ObjectNode, Value};
pub use compat::{
    escape_scalar_for_generate, export_entry, export_key, normalize_scalar_for_parse,
    restore_compat_operators,
};
pub use error::Hoi4ParserError;
pub use generator::generate_document;
pub use nested::{decode_nested_quoted, encode_nested_quoted};
pub use perf::{benchmark_round_trip, BenchReport};
pub use tokenizer::Token;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    root: Value,
    source: String,
}

impl Document {
    pub fn new(root: Value, source: impl Into<String>) -> Self {
        Self {
            root,
            source: source.into(),
        }
    }

    pub fn root(&self) -> &Value {
        &self.root
    }

    pub fn as_source(&self) -> &str {
        &self.source
    }
}

/// 解析 Paradox 文本为文档结构。
///
/// 当前已接入 tokenizer 与 parser，支持基础键值与嵌套对象解析。
pub fn parse(input: &str) -> Result<Document, Hoi4ParserError> {
    if input.trim().is_empty() {
        return Err(Hoi4ParserError::Parse {
            message: "输入为空，无法解析".to_string(),
        });
    }

    let tokens = tokenizer::tokenize(input)?;
    let root = parser::parse_root(&tokens)?;
    Ok(Document::new(root, input))
}

/// 将文档结构还原为 Paradox 文本。
///
/// 当前由 generator 模块根据 AST 直接输出。
pub fn generate(document: &Document) -> Result<String, Hoi4ParserError> {
    if document.as_source().is_empty() {
        // 允许空 source，但 AST 必须可生成。
        if matches!(document.root(), Value::Object(object) if object.entries().is_empty()) {
            return Err(Hoi4ParserError::Generate {
                message: "文档为空，无法还原".to_string(),
            });
        }
    }
    let rendered = generator::generate_document(document)?;
    Ok(restore_compat_operators(&rendered))
}

#[cfg(test)]
mod tests {
    use super::{generate, parse, Entry, EntryMetadata, ObjectNode, Value};

    #[test]
    fn parse_and_generate_round_trip_source() {
        let source = "country = { tag = CHI }";
        let doc = parse(source).expect("parse should succeed");
        let output = generate(&doc).expect("generate should succeed");
        let expected = "country = {\n\ttag = CHI\n}";
        assert_eq!(output, expected);
        assert!(matches!(doc.root(), Value::Object(_)));
    }

    #[test]
    fn parse_empty_input_should_fail() {
        let result = parse("   \n\t");
        assert!(result.is_err());
    }

    #[test]
    fn ast_object_should_keep_duplicate_key_metadata() {
        let mut object = ObjectNode::default();
        object.push(Entry::new("name", Value::Scalar("alpha".to_string())));
        object.push(Entry::with_metadata(
            "name",
            Value::Scalar("beta".to_string()),
            EntryMetadata {
                duplicate_index: Some(1),
                duplicate_suffix: Some("$$1".to_string()),
                nested_quoted: false,
            },
        ));

        assert_eq!(object.entries().len(), 2);
        assert_eq!(object.entries()[1].key(), "name");
        assert_eq!(object.entries()[1].metadata().duplicate_index, Some(1));
    }

    #[test]
    fn round_trip_should_keep_semantic_ast() {
        let source = "country = { name = \"China\" name = \"PRC\" effect = \"set_var = { key = \\\"x\\\" value = 1 }\" }";
        let first_doc = parse(source).expect("first parse should succeed");
        let regenerated = generate(&first_doc).expect("generate should succeed");
        let second_doc = parse(&regenerated).expect("second parse should succeed");

        assert_eq!(first_doc.root(), second_doc.root());
    }
}
