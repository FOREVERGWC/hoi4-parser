use crate::{Entry, Value};
use std::borrow::Cow;

pub fn export_key(entry: &Entry, with_duplicate_suffix: bool) -> String {
    if with_duplicate_suffix {
        if let Some(suffix) = &entry.metadata().duplicate_suffix {
            let mut out = String::with_capacity(entry.key().len() + suffix.len());
            out.push_str(entry.key());
            out.push_str(suffix);
            return out;
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

pub fn normalize_scalar_for_parse(raw: &str) -> Cow<'_, str> {
    let has_compat_markers = raw.contains('%')
        || raw.contains('!')
        || raw.contains('&')
        || raw.contains("--")
        || raw.ends_with(',');
    if !has_compat_markers {
        return Cow::Borrowed(raw);
    }

    let decoded = decode_parse_compat_markers(raw);
    let stripped = strip_trailing_comma_typo(&decoded);
    Cow::Owned(stripped.into_owned())
}

/// 原稿里常见笔误：`trait_cautious,` 换行下一项；tokenizer 会把逗号吃进 ident，生成会多出逗号行。
/// 仅在去掉末尾逗号后整段仍像**单个**脚本 token 时去掉（避免误伤含括号、空格等表达式）。
fn strip_trailing_comma_typo(s: &str) -> Cow<'_, str> {
    let Some(body) = s.strip_suffix(',') else {
        return Cow::Borrowed(s);
    };
    if body.is_empty() {
        return Cow::Borrowed(s);
    }
    let looks_like_single_token = body
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ':' | '\'' | '’'));
    if looks_like_single_token {
        Cow::Owned(body.to_string())
    } else {
        Cow::Borrowed(s)
    }
}

pub fn escape_scalar_for_generate(raw: &str) -> String {
    escape_scalar_for_generate_cow(raw).into_owned()
}

pub fn escape_scalar_for_generate_cow(raw: &str) -> Cow<'_, str> {
    let needs_escape = raw.contains(':')
        || raw.contains('[')
        || raw.contains(']')
        || raw.contains('>')
        || raw.contains('<')
        || raw.contains("==");
    if !needs_escape {
        return Cow::Borrowed(raw);
    }
    let mut out = String::with_capacity(raw.len() + 8);
    let bytes = raw.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let remain = &raw[i..];
        if remain.starts_with(">=") {
            out.push_str("&gte;");
            i += 2;
            continue;
        }
        if remain.starts_with("<=") {
            out.push_str("&lte;");
            i += 2;
            continue;
        }
        if remain.starts_with("==") {
            out.push_str("&eqeq;");
            i += 2;
            continue;
        }

        match bytes[i] {
            b':' => {
                out.push_str("--");
                i += 1;
            }
            b'[' => {
                out.push_str("%%");
                i += 1;
            }
            b']' => {
                out.push_str("!!");
                i += 1;
            }
            b'>' => {
                out.push_str("&gt;");
                i += 1;
            }
            b'<' => {
                out.push_str("&lt;");
                i += 1;
            }
            _ => {
                let ch = remain.chars().next().expect("non-empty slice");
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    Cow::Owned(out)
}

fn decode_parse_compat_markers(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut i = 0usize;
    while i < raw.len() {
        let remain = &raw[i..];
        let (matched, replacement): (usize, &str) = if remain.starts_with("%%") {
            (2, "[")
        } else if remain.starts_with("!!") {
            (2, "]")
        } else if remain.starts_with("--") {
            (2, ":")
        } else if remain.starts_with("&gte;") {
            (5, ">=")
        } else if remain.starts_with("&lte;") {
            (5, "<=")
        } else if remain.starts_with("&eqeq;") {
            (6, "==")
        } else if remain.starts_with("&gt;") {
            (4, ">")
        } else if remain.starts_with("&lt;") {
            (4, "<")
        } else {
            (0, "")
        };

        if matched > 0 {
            out.push_str(replacement);
            i += matched;
            continue;
        }

        let ch = remain.chars().next().expect("non-empty slice");
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

pub fn restore_compat_operators(rendered: &str) -> String {
    if !rendered.contains('&')
        && !rendered.contains('%')
        && !rendered.contains('!')
        && !rendered.contains("--")
        && !rendered.contains('"')
    {
        return rendered.to_string();
    }

    let mut out = String::with_capacity(rendered.len());
    for (idx, line) in rendered.lines().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        let restored = restore_compat_markers_line(line);
        let unquoted = unquote_macro_line_generic(&restored);
        out.push_str(&unquoted);
    }
    out
}

fn restore_compat_markers_line(line: &str) -> Cow<'_, str> {
    let needs_restore =
        line.contains('&') || line.contains('%') || line.contains('!') || line.contains("--");
    if !needs_restore {
        return Cow::Borrowed(line);
    }

    let mut out = String::with_capacity(line.len());
    let mut i = 0usize;
    while i < line.len() {
        let remain = &line[i..];
        let (matched, replacement): (usize, &str) = if remain.starts_with("= &gte;") {
            (7, ">=")
        } else if remain.starts_with("= &lte;") {
            (7, "<=")
        } else if remain.starts_with("= &eqeq;") {
            (8, "==")
        } else if remain.starts_with("= &gt;") {
            (6, ">")
        } else if remain.starts_with("= &lt;") {
            (6, "<")
        } else if remain.starts_with("&gt;") {
            (4, ">")
        } else if remain.starts_with("&lt;") {
            (4, "<")
        } else if remain.starts_with("%%") {
            (2, "[")
        } else if remain.starts_with("!!") {
            (2, "]")
        } else if remain.starts_with("--") {
            (2, ":")
        } else {
            (0, "")
        };

        if matched > 0 {
            out.push_str(replacement);
            i += matched;
            continue;
        }

        let ch = remain.chars().next().expect("non-empty slice");
        out.push(ch);
        i += ch.len_utf8();
    }
    Cow::Owned(out)
}

fn unquote_macro_line_generic(line: &str) -> Cow<'_, str> {
    let Some(eq_pos) = line.find('=') else {
        return Cow::Borrowed(line);
    };
    let value_part = line[eq_pos + 1..].trim();
    if value_part.len() < 2 || !value_part.starts_with('"') || !value_part.ends_with('"') {
        return Cow::Borrowed(line);
    }
    let token = &value_part[1..value_part.len() - 1];
    if !is_macro_token(token) {
        return Cow::Borrowed(line);
    }
    let left = &line[..eq_pos + 1];
    let mut out = String::with_capacity(left.len() + token.len() + 1);
    out.push_str(left);
    out.push(' ');
    out.push_str(token);
    Cow::Owned(out)
}

fn is_macro_token(token: &str) -> bool {
    !token.is_empty()
        && token.contains('_')
        && token.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
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
    fn should_strip_trailing_comma_on_trait_like_token() {
        assert_eq!(
            normalize_scalar_for_parse("trait_cautious,"),
            "trait_cautious"
        );
    }

    #[test]
    fn should_not_strip_comma_when_token_has_structure() {
        assert_eq!(normalize_scalar_for_parse("foo(x),"), "foo(x),");
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

    #[test]
    fn should_unquote_macro_tokens_generically() {
        let text =
            "foo = \"BLITZKRIEG_NAME\"\nbar = \"BLITZKRIEG_DESC\"\nany = \"GFX_select_date_1939\"";
        let restored = restore_compat_operators(text);
        assert_eq!(
            restored,
            "foo = BLITZKRIEG_NAME\nbar = BLITZKRIEG_DESC\nany = GFX_select_date_1939"
        );
    }
}
