use crate::{Entry, Value};

pub fn export_key(entry: &Entry, with_duplicate_suffix: bool) -> String {
    if with_duplicate_suffix {
        if let Some(suffix) = &entry.metadata().duplicate_suffix {
            return format!("{}{}", entry.key(), suffix);
        }
    }
    entry.key().to_string()
}

pub fn export_entry(entry: &Entry, with_duplicate_suffix: bool) -> (String, Value) {
    (
        export_key(entry, with_duplicate_suffix),
        entry.value().clone(),
    )
}

pub fn normalize_scalar_for_parse(raw: &str) -> String {
    raw.replace("%%", "[")
        .replace("!!", "]")
        .replace("--", ":")
        .replace("&gte;", ">=")
        .replace("&lte;", "<=")
        .replace("&eqeq;", "==")
        .replace("&gt;", ">")
        .replace("&lt;", "<")
}

pub fn escape_scalar_for_generate(raw: &str) -> String {
    raw.replace(":", "--")
        .replace("[", "%%")
        .replace("]", "!!")
        .replace(">=", "&gte;")
        .replace("<=", "&lte;")
        .replace("==", "&eqeq;")
        .replace(">", "&gt;")
        .replace("<", "&lt;")
}

pub fn restore_compat_operators(rendered: &str) -> String {
    rendered
        .replace("= &gte;", ">=")
        .replace("= &lte;", "<=")
        .replace("= &eqeq;", "==")
        .replace("= &gt;", ">")
        .replace("= &lt;", "<")
        .replace("%%", "[")
        .replace("!!", "]")
        .replace("--", ":")
}

#[cfg(test)]
mod tests {
    use super::{
        escape_scalar_for_generate, export_key, normalize_scalar_for_parse,
        restore_compat_operators,
    };
    use crate::{Entry, EntryMetadata, Value};

    #[test]
    fn should_append_suffix_when_compat_enabled() {
        let entry = Entry::with_metadata(
            "name",
            Value::Scalar("beta".to_string()),
            EntryMetadata {
                duplicate_index: Some(1),
                duplicate_suffix: Some("$$1".to_string()),
                nested_quoted: false,
            },
        );

        assert_eq!(export_key(&entry, true), "name$$1");
        assert_eq!(export_key(&entry, false), "name");
    }

    #[test]
    fn should_escape_and_restore_scalar_symbols() {
        let escaped = escape_scalar_for_generate("a:[x] >= b");
        assert_eq!(escaped, "a--%%x!! &gte; b");

        let normalized = normalize_scalar_for_parse(&escaped);
        assert_eq!(normalized, "a:[x] >= b");
    }

    #[test]
    fn should_restore_operator_style_output() {
        let text = "value = &gt;\nvalue = &lte;";
        let restored = restore_compat_operators(text);
        assert_eq!(restored, "value >\nvalue <=");
    }

    #[test]
    fn should_restore_bracket_and_colon_symbols() {
        let text = "token = %%From.GetID!!\npath = core--china";
        let restored = restore_compat_operators(text);
        assert_eq!(restored, "token = [From.GetID]\npath = core:china");
    }
}
