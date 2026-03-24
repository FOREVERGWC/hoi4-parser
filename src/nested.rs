pub fn decode_nested_quoted(raw: &str) -> Option<String> {
    if !raw.starts_with('"') || !raw.ends_with('"') {
        return None;
    }
    if !raw.contains("\\\"") {
        return None;
    }

    let inner = &raw[1..raw.len() - 1];
    Some(inner.replace("\\\"", "\""))
}

pub fn encode_nested_quoted(raw: &str) -> String {
    format!("\"{}\"", raw.replace('"', "\\\"").replace('\n', " "))
}

#[cfg(test)]
mod tests {
    use super::{decode_nested_quoted, encode_nested_quoted};

    #[test]
    fn should_decode_nested_string() {
        let decoded = decode_nested_quoted("\"a = { b = \\\"x\\\" }\"").expect("should decode");
        assert_eq!(decoded, "a = { b = \"x\" }");
    }

    #[test]
    fn should_encode_nested_string() {
        let encoded = encode_nested_quoted("a = { b = \"x\" }");
        assert_eq!(encoded, "\"a = { b = \\\"x\\\" }\"");
    }
}
