use crate::compat::escape_scalar_for_generate;
use crate::{encode_nested_quoted, Document, Entry, Hoi4ParserError, Value};

pub fn generate_document(document: &Document) -> Result<String, Hoi4ParserError> {
    match document.root() {
        Value::Object(root) => {
            let lines = root
                .entries()
                .iter()
                .map(|entry| generate_entry(entry, 0, false))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(lines.join("\n"))
        }
        other => generate_value(other, 0, true),
    }
}

fn generate_value(
    value: &Value,
    indent: usize,
    inline_object: bool,
) -> Result<String, Hoi4ParserError> {
    match value {
        Value::Scalar(s) => Ok(escape_scalar_for_generate(s)),
        Value::Array(items) => {
            let rendered = items
                .iter()
                .map(|item| generate_value(item, indent, true))
                .collect::<Result<Vec<_>, _>>()?
                .join(" ");
            Ok(format!("{{ {rendered} }}"))
        }
        Value::Object(object) => {
            let entries = object.entries();
            if entries.is_empty() {
                return Ok("{}".to_string());
            }

            if inline_object {
                let rendered = entries
                    .iter()
                    .map(|entry| generate_entry(entry, indent, true))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(" ");
                return Ok(format!("{{ {rendered} }}"));
            }

            let mut lines = Vec::new();
            for entry in entries {
                lines.push(format!(
                    "{}{}",
                    "\t".repeat(indent),
                    generate_entry(entry, indent, false)?
                ));
            }
            Ok(lines.join("\n"))
        }
    }
}

fn generate_entry(
    entry: &Entry,
    indent: usize,
    inline_object: bool,
) -> Result<String, Hoi4ParserError> {
    if entry.metadata().nested_quoted {
        let inner = generate_nested_payload(entry.value(), indent + 1)?;
        let encoded = encode_nested_quoted(&inner);
        return Ok(format!("{} = {}", entry.key(), encoded));
    }

    match entry.value() {
        Value::Object(obj) if !inline_object && !obj.entries().is_empty() => {
            let body = generate_value(entry.value(), indent + 1, false)?;
            Ok(format!(
                "{} = {{\n{}\n{}}}",
                entry.key(),
                body,
                "\t".repeat(indent)
            ))
        }
        _ => {
            let value = generate_value(entry.value(), indent + 1, true)?;
            Ok(format!("{} = {}", entry.key(), value))
        }
    }
}

fn generate_nested_payload(value: &Value, indent: usize) -> Result<String, Hoi4ParserError> {
    match value {
        Value::Object(object) => {
            let rendered = object
                .entries()
                .iter()
                .map(|entry| generate_entry(entry, indent, true))
                .collect::<Result<Vec<_>, _>>()?
                .join(" ");
            Ok(rendered)
        }
        other => generate_value(other, indent, true),
    }
}

#[cfg(test)]
mod tests {
    use super::generate_document;
    use crate::{Document, Entry, EntryMetadata, ObjectNode, Value};

    #[test]
    fn should_generate_basic_object() {
        let mut root = ObjectNode::default();
        root.push(Entry::new("tag", Value::Scalar("CHI".to_string())));
        let document = Document::new(Value::Object(root), "tag = CHI");

        let rendered = generate_document(&document).expect("generate should succeed");
        assert_eq!(rendered, "tag = CHI");
    }

    #[test]
    fn should_generate_nested_quoted_when_flag_enabled() {
        let mut nested = ObjectNode::default();
        nested.push(Entry::new("name", Value::Scalar("\"x\"".to_string())));

        let entry = Entry::with_metadata(
            "effect",
            Value::Object(nested),
            EntryMetadata {
                duplicate_index: None,
                duplicate_suffix: None,
                nested_quoted: true,
            },
        );

        let mut root = ObjectNode::default();
        root.push(entry);
        let document = Document::new(Value::Object(root), "");

        let rendered = generate_document(&document).expect("generate should succeed");
        assert_eq!(rendered, "effect = \"name = \\\"x\\\"\"");
    }
}
