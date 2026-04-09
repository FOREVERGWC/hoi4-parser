use std::borrow::Cow;

use crate::compat::escape_scalar_for_generate_cow;
use crate::tokenizer::Token;
use crate::{decode_nested_quoted, encode_nested_quoted, Document, Entry, Hoi4ParserError, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layout {
    Inline,
    Multiline,
}

pub fn generate_document(document: &Document) -> Result<String, Hoi4ParserError> {
    match document.root() {
        Value::Object(root) => {
            let entries = root.entries();
            let mut out = String::new();
            for (idx, entry) in entries.iter().enumerate() {
                if idx > 0 {
                    out.push('\n');
                }
                out.push_str(&generate_entry_with_options(entry, 0, false, true)?);
            }
            Ok(out)
        }
        Value::AnonymousObject(root) => render_object_node(root, 0, false, true),
        other => generate_value_with_options(other, 0, true, true),
    }
}

fn generate_value_with_options(
    value: &Value,
    indent: usize,
    inline_object: bool,
    trim_numeric_scalars: bool,
) -> Result<String, Hoi4ParserError> {
    match value {
        Value::Scalar(s) => {
            let rendered = render_scalar_for_generate(s, trim_numeric_scalars)
                .as_deref()
                .map(|text| escape_scalar_for_generate_cow(text).into_owned())
                .unwrap_or_else(|| escape_scalar_for_generate_cow(s).into_owned());
            Ok(rendered)
        }
        Value::Array(items) => {
            let mut rendered = String::new();
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    rendered.push(' ');
                }
                rendered.push_str(&generate_value_with_options(
                    item,
                    indent,
                    true,
                    trim_numeric_scalars,
                )?);
            }
            Ok(wrap_inline_braces(&rendered))
        }
        Value::Object(object) | Value::AnonymousObject(object) => {
            render_object_node(object, indent, inline_object, trim_numeric_scalars)
        }
    }
}

fn render_object_node(
    object: &crate::ObjectNode,
    indent: usize,
    inline_object: bool,
    trim_numeric_scalars: bool,
) -> Result<String, Hoi4ParserError> {
    let entries = object.entries();
    if entries.is_empty() {
        return Ok("{}".to_string());
    }

    if inline_object {
        let mut rendered = String::new();
        for (idx, entry) in entries.iter().enumerate() {
            if idx > 0 {
                rendered.push(' ');
            }
            rendered.push_str(&generate_entry_with_options(
                entry,
                indent,
                true,
                trim_numeric_scalars,
            )?);
        }
        return Ok(wrap_inline_braces(&rendered));
    }

    let prefix = "\t".repeat(indent);
    let mut out = String::new();
    for (idx, entry) in entries.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        out.push_str(&prefix);
        out.push_str(&generate_entry_with_options(
            entry,
            indent,
            false,
            trim_numeric_scalars,
        )?);
    }
    Ok(out)
}

fn normalize_quoted_scalar_like_java(raw: &str) -> Cow<'_, str> {
    if raw.len() >= 2
        && raw.starts_with('"')
        && raw.ends_with('"')
        && raw.contains('_')
        && !raw.contains(' ')
        && !raw.contains('/')
    {
        return Cow::Owned(raw[1..raw.len() - 1].to_string());
    }
    Cow::Borrowed(raw)
}

fn canonicalize_script_like_quoted_scalar(raw: &str) -> Cow<'_, str> {
    if !(raw.starts_with('"') && raw.ends_with('"') && raw.contains("\\\"") && raw.contains('=')) {
        return Cow::Borrowed(raw);
    }
    let Some(decoded) = decode_nested_quoted(raw) else {
        return Cow::Borrowed(raw);
    };
    let Ok(tokens) = crate::tokenizer::tokenize(&decoded) else {
        return Cow::Borrowed(raw);
    };

    let mut canonical = String::new();
    let mut has_parts = false;
    for token in tokens {
        let part = match token {
            Token::Ident(s) | Token::StringLiteral(s) => Some(s),
            Token::Equals => Some("="),
            Token::LBrace => Some("{"),
            Token::RBrace => Some("}"),
            Token::Newline => None,
        };
        if let Some(text) = part {
            if has_parts {
                canonical.push(' ');
            } else {
                has_parts = true;
            }
            canonical.push_str(text);
        }
    }

    if !has_parts {
        return Cow::Borrowed(raw);
    }
    Cow::Owned(encode_nested_quoted(&canonical))
}

fn render_color_scalar_block_body(text: &str, indent: usize) -> Option<String> {
    let (keyword, inner) = if let Some(inner) = text
        .strip_prefix("rgb { ")
        .and_then(|s| s.strip_suffix(" }"))
    {
        ("rgb", inner)
    } else if let Some(inner) = text
        .strip_prefix("HSV { ")
        .and_then(|s| s.strip_suffix(" }"))
    {
        ("HSV", inner)
    } else if let Some(inner) = text
        .strip_prefix("hsv { ")
        .and_then(|s| s.strip_suffix(" }"))
    {
        ("hsv", inner)
    } else {
        return None;
    };
    if inner.trim().is_empty() {
        return None;
    }
    let prefix = "\t".repeat(indent);
    let mut out = String::new();
    out.push_str(&prefix);
    out.push_str(keyword);
    for part in inner.split_whitespace() {
        out.push('\n');
        out.push_str(&prefix);
        out.push_str(part);
    }
    Some(out)
}

fn generate_entry_with_options(
    entry: &Entry,
    indent: usize,
    inline_object: bool,
    trim_numeric_scalars: bool,
) -> Result<String, Hoi4ParserError> {
    if entry.metadata().nested_quoted {
        let inner = generate_nested_payload(entry.value(), indent + 1, trim_numeric_scalars)?;
        let encoded = encode_nested_quoted(&inner);
        return Ok(join_key_value(entry.key(), &encoded));
    }

    match decide_entry_value_layout(entry.value(), !inline_object, indent + 1) {
        Layout::Multiline => {
            let body =
                render_multiline_entry_value(entry.value(), indent + 1, trim_numeric_scalars)?;
            Ok(wrap_keyed_block(entry.key(), &body, indent))
        }
        Layout::Inline => {
            if let Value::Scalar(text) = entry.value() {
                let canonicalized = canonicalize_script_like_quoted_scalar(text);
                let normalized = normalize_quoted_scalar_like_java(&canonicalized);
                let rendered_scalar = render_entry_scalar_for_generate(
                    entry.key(),
                    normalized.as_ref(),
                    trim_numeric_scalars,
                );
                return Ok(join_key_value(
                    entry.key(),
                    &escape_scalar_for_generate_cow(rendered_scalar.as_ref()),
                ));
            }
            let value =
                generate_value_with_options(entry.value(), indent + 1, true, trim_numeric_scalars)?;
            Ok(join_key_value(entry.key(), &value))
        }
    }
}

enum ArrayItemKind {
    ScalarThenEqualsFollowingComposite,
    AttachedOperator,
    ScalarEqualsFollowingComposite,
    SplitScalarList,
    BareMultilineValue,
    Default,
}

fn render_multiline_array_block_with_options(
    items: &[Value],
    indent: usize,
    trim_numeric_scalars: bool,
) -> Result<String, Hoi4ParserError> {
    let prefix = "\t".repeat(indent);
    if is_single_expression_array(items) {
        let mut expr = String::new();
        let mut has_any = false;
        for item in items {
            if let Value::Scalar(text) = item {
                if has_any {
                    expr.push(' ');
                } else {
                    has_any = true;
                }
                let rendered = render_scalar_for_generate(text, trim_numeric_scalars)
                    .map(|cow| cow.into_owned())
                    .unwrap_or_else(|| text.clone());
                expr.push_str(&rendered);
            }
        }
        return Ok(concat_with_prefix(&prefix, &expr));
    }

    let mut out = String::new();
    let mut is_first_line = true;
    let mut i = 0usize;
    while i < items.len() {
        match classify_array_item(items, i) {
            ArrayItemKind::ScalarThenEqualsFollowingComposite => {
                if let Some(combined) = render_scalar_then_equals_following_object(
                    items,
                    i,
                    indent,
                    trim_numeric_scalars,
                )? {
                    append_block(&mut out, &combined, &mut is_first_line);
                    i += 3;
                    continue;
                }
            }
            ArrayItemKind::AttachedOperator => {
                if let Some(combined) =
                    render_attached_operator_pair(items, i, &prefix, trim_numeric_scalars)
                {
                    append_line(&mut out, &combined, &mut is_first_line);
                    i += 2;
                    continue;
                }
            }
            ArrayItemKind::ScalarEqualsFollowingComposite => {
                if let Some(combined) =
                    render_scalar_equals_following_object(items, i, indent, trim_numeric_scalars)?
                {
                    append_block(&mut out, &combined, &mut is_first_line);
                    i += 2;
                    continue;
                }
            }
            ArrayItemKind::SplitScalarList => {
                if let Some(split_block) = render_split_scalar_list_item(&items[i], &prefix) {
                    append_block(&mut out, &split_block, &mut is_first_line);
                    i += 1;
                    continue;
                }
            }
            ArrayItemKind::BareMultilineValue => {
                if let Some(multiline_block) =
                    render_bare_multiline_value(&items[i], indent, trim_numeric_scalars)?
                {
                    append_block(&mut out, &multiline_block, &mut is_first_line);
                    i += 1;
                    continue;
                }
            }
            ArrayItemKind::Default => {}
        }

        let rendered = generate_value_with_options(&items[i], indent, true, trim_numeric_scalars)?;
        let line = concat_with_prefix(&prefix, &rendered);
        append_line(&mut out, &line, &mut is_first_line);
        i += 1;
    }
    Ok(out)
}

fn classify_array_item(items: &[Value], index: usize) -> ArrayItemKind {
    if is_scalar_then_equals_following_composite(items, index) {
        return ArrayItemKind::ScalarThenEqualsFollowingComposite;
    }
    if is_attached_operator_pair(items, index) {
        return ArrayItemKind::AttachedOperator;
    }
    if is_scalar_equals_following_composite(items, index) {
        return ArrayItemKind::ScalarEqualsFollowingComposite;
    }
    if matches!(&items[index], Value::Scalar(text) if text.contains(' ')) {
        return ArrayItemKind::SplitScalarList;
    }
    if matches!(
        decide_array_item_layout(&items[index], indent_of_array_item_hint()),
        Layout::Multiline
    ) {
        return ArrayItemKind::BareMultilineValue;
    }
    ArrayItemKind::Default
}

fn indent_of_array_item_hint() -> usize {
    0
}

fn render_attached_operator_pair(
    items: &[Value],
    index: usize,
    prefix: &str,
    trim_numeric_scalars: bool,
) -> Option<String> {
    if index + 1 >= items.len() {
        return None;
    }
    let Value::Scalar(left) = &items[index] else {
        return None;
    };
    let Value::Scalar(right) = &items[index + 1] else {
        return None;
    };
    let op = right.chars().next()?;
    if op != '<' && op != '>' {
        return None;
    }
    let tail = &right[op.len_utf8()..];
    if tail.is_empty() || !is_simple_identifier(tail) {
        return None;
    }
    let rendered_tail = render_scalar_for_generate(tail, trim_numeric_scalars)
        .map(|cow| cow.into_owned())
        .unwrap_or_else(|| tail.to_string());
    // 统一为 "a op b"（比较符两侧有空格），与 fixture / 用户期望一致
    let mut line = String::with_capacity(prefix.len() + left.len() + rendered_tail.len() + 3);
    line.push_str(prefix);
    line.push_str(left);
    line.push(' ');
    line.push(op);
    line.push(' ');
    line.push_str(&rendered_tail);
    Some(line)
}

fn is_attached_operator_pair(items: &[Value], index: usize) -> bool {
    if index + 1 >= items.len() {
        return false;
    }
    let Value::Scalar(left) = &items[index] else {
        return false;
    };
    if left.is_empty() {
        return false;
    }
    let Value::Scalar(right) = &items[index + 1] else {
        return false;
    };
    let Some(op) = right.chars().next() else {
        return false;
    };
    if op != '<' && op != '>' {
        return false;
    }
    let tail = &right[op.len_utf8()..];
    !tail.is_empty() && is_simple_identifier(tail)
}

fn render_scalar_equals_following_object(
    items: &[Value],
    index: usize,
    indent: usize,
    trim_numeric_scalars: bool,
) -> Result<Option<String>, Hoi4ParserError> {
    if index + 1 >= items.len() {
        return Ok(None);
    }
    let Value::Scalar(prefix_text) = &items[index] else {
        return Ok(None);
    };
    if !prefix_text.trim_end().ends_with('=') {
        return Ok(None);
    }
    let prefix_without_eq = prefix_text.trim_end().trim_end_matches('=').trim();
    if prefix_without_eq.is_empty() {
        return Ok(None);
    }
    let composite = &items[index + 1];
    Ok(Some(render_prefixed_composite_block(
        prefix_without_eq,
        composite,
        indent,
        trim_numeric_scalars,
    )?))
}

fn render_scalar_then_equals_following_object(
    items: &[Value],
    index: usize,
    indent: usize,
    trim_numeric_scalars: bool,
) -> Result<Option<String>, Hoi4ParserError> {
    if index + 2 >= items.len() {
        return Ok(None);
    }
    let Value::Scalar(prefix_text) = &items[index] else {
        return Ok(None);
    };
    let Value::Scalar(eq) = &items[index + 1] else {
        return Ok(None);
    };
    if eq.trim() != "=" {
        return Ok(None);
    }
    let prefix_text = prefix_text.trim();
    if prefix_text.is_empty() {
        return Ok(None);
    }
    let composite = &items[index + 2];
    Ok(Some(render_prefixed_composite_block(
        prefix_text,
        composite,
        indent,
        trim_numeric_scalars,
    )?))
}

fn render_prefixed_composite_block(
    prefix_text: &str,
    composite: &Value,
    indent: usize,
    trim_numeric_scalars: bool,
) -> Result<String, Hoi4ParserError> {
    let mut out = String::new();
    let mut is_first_line = true;
    let indent_prefix = "\t".repeat(indent);
    let child_prefix = "\t".repeat(indent + 1);
    let prefix_parts: Vec<&str> = prefix_text.split_whitespace().collect();
    let open_key =
        if prefix_parts.len() >= 2 && prefix_parts.iter().all(|p| is_simple_identifier(p)) {
            for part in &prefix_parts[..prefix_parts.len() - 1] {
                let line = concat_with_prefix(&indent_prefix, part);
                append_line(&mut out, &line, &mut is_first_line);
            }
            prefix_parts[prefix_parts.len() - 1].to_string()
        } else {
            prefix_text.to_string()
        };

    let mut open_line = String::with_capacity(indent_prefix.len() + open_key.len() + 5);
    open_line.push_str(&indent_prefix);
    open_line.push_str(&open_key);
    open_line.push_str(" = {");
    append_line(&mut out, &open_line, &mut is_first_line);

    match composite {
        Value::Object(object) | Value::AnonymousObject(object) => {
            for entry in object.entries() {
                let rendered =
                    generate_entry_with_options(entry, indent + 1, false, trim_numeric_scalars)?;
                let line = concat_with_prefix(&child_prefix, &rendered);
                append_line(&mut out, &line, &mut is_first_line);
            }
        }
        Value::Array(array_items) => {
            let array_block = render_multiline_array_block_with_options(
                array_items,
                indent + 1,
                trim_numeric_scalars,
            )?;
            append_block(&mut out, &array_block, &mut is_first_line);
        }
        _ => {
            return Err(Hoi4ParserError::Generate {
                message: "前缀等号块仅支持对象或数组".to_string(),
            });
        }
    }
    let close = close_brace_line(&indent_prefix);
    append_line(&mut out, &close, &mut is_first_line);
    Ok(out)
}

fn is_scalar_equals_following_composite(items: &[Value], index: usize) -> bool {
    if index + 1 >= items.len() {
        return false;
    }
    let Value::Scalar(prefix) = &items[index] else {
        return false;
    };
    if !prefix.trim_end().ends_with('=') {
        return false;
    }
    matches!(
        &items[index + 1],
        Value::Object(_) | Value::AnonymousObject(_) | Value::Array(_)
    )
}

fn is_scalar_then_equals_following_composite(items: &[Value], index: usize) -> bool {
    if index + 2 >= items.len() {
        return false;
    }
    let Value::Scalar(prefix) = &items[index] else {
        return false;
    };
    if prefix.trim().is_empty() {
        return false;
    }
    let Value::Scalar(eq) = &items[index + 1] else {
        return false;
    };
    if eq.trim() != "=" {
        return false;
    }
    matches!(
        &items[index + 2],
        Value::Object(_) | Value::AnonymousObject(_) | Value::Array(_)
    )
}

fn is_single_expression_array(items: &[Value]) -> bool {
    if items.len() < 3 {
        return false;
    }
    // 纯标识符列表应逐行输出，避免被压成 "GER SLO" 这种单行。
    if items
        .iter()
        .all(|item| matches!(item, Value::Scalar(text) if is_simple_identifier(text)))
    {
        return false;
    }
    let mut has_operator = false;
    for item in items {
        let Value::Scalar(text) = item else {
            return false;
        };
        if text.contains(' ') {
            return false;
        }
        if matches!(text.as_str(), ">" | "<" | ">=" | "<=" | "==" | "!=") {
            has_operator = true;
        }
    }
    has_operator
}

fn is_simple_identifier(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ':' | '\'' | '’'))
}

fn render_split_scalar_list_item(item: &Value, prefix: &str) -> Option<String> {
    let Value::Scalar(text) = item else {
        return None;
    };
    if !text.contains(' ') {
        return None;
    }
    let tokens = split_tokens_preserving_quotes(text)?;
    if tokens.len() < 2 || !tokens.iter().all(|token| is_list_token(token)) {
        return None;
    }
    let mut out = String::new();
    let mut first = true;
    for token in tokens {
        if first {
            first = false;
        } else {
            out.push('\n');
        }
        out.push_str(prefix);
        out.push_str(&token);
    }
    Some(out)
}

fn split_tokens_preserving_quotes(text: &str) -> Option<Vec<String>> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    // None = 不在引号内；Some(true) = ASCII "..."；Some(false) = 弯引号 “...” 或 ‘...’
    let mut in_quote: Option<bool> = None;
    let mut escaped = false;

    for ch in text.chars() {
        if let Some(ascii) = in_quote {
            current.push(ch);
            if ascii {
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_quote = None;
                }
            } else {
                // 弯引号对：起笔 “(201c) 或 ‘(2018)，收笔 ”(201d) 或 ’(2019)
                if matches!(ch, '\u{201d}' | '\u{2019}') {
                    in_quote = None;
                }
            }
            continue;
        }

        if ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            continue;
        }
        if ch == ',' {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            tokens.push(",".to_string());
            continue;
        }

        if ch == '"' {
            in_quote = Some(true);
        } else if ch == '\u{201c}' || ch == '\u{2018}' {
            in_quote = Some(false);
        }
        current.push(ch);
    }

    if in_quote.is_some() {
        return None;
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    Some(tokens)
}

fn is_list_token(token: &str) -> bool {
    token == "," || is_simple_identifier(token) || is_quoted_token(token)
}

fn is_quoted_token(token: &str) -> bool {
    if token.len() < 2 {
        return false;
    }
    let first = token.chars().next().unwrap();
    let last = token.chars().last().unwrap();
    matches!((first, last), ('"', '"'))
        || matches!((first, last), ('\u{201c}', '\u{201d}')) // “…”
        || matches!((first, last), ('\u{2018}', '\u{2019}')) // ‘…’
}

fn render_bare_multiline_value(
    item: &Value,
    indent: usize,
    trim_numeric_scalars: bool,
) -> Result<Option<String>, Hoi4ParserError> {
    match item {
        Value::Object(object) | Value::AnonymousObject(object) => {
            let prefix = "\t".repeat(indent);
            let child_prefix = "\t".repeat(indent + 1);
            let mut out = String::new();
            let mut is_first_line = true;
            let open = open_anonymous_block_line(&prefix);
            append_line(&mut out, &open, &mut is_first_line);
            for entry in object.entries() {
                let rendered =
                    generate_entry_with_options(entry, indent + 1, false, trim_numeric_scalars)?;
                let line = concat_with_prefix(&child_prefix, &rendered);
                append_line(&mut out, &line, &mut is_first_line);
            }
            let close = close_brace_line(&prefix);
            append_line(&mut out, &close, &mut is_first_line);
            Ok(Some(out))
        }
        Value::Array(items) => {
            let prefix = "\t".repeat(indent);
            let mut out = String::new();
            let mut is_first_line = true;
            let open = open_anonymous_block_line(&prefix);
            append_line(&mut out, &open, &mut is_first_line);
            let body =
                render_multiline_array_block_with_options(items, indent + 1, trim_numeric_scalars)?;
            append_block(&mut out, &body, &mut is_first_line);
            let close = close_brace_line(&prefix);
            append_line(&mut out, &close, &mut is_first_line);
            Ok(Some(out))
        }
        _ => Ok(None),
    }
}

fn decide_entry_value_layout(value: &Value, allow_multiline: bool, indent: usize) -> Layout {
    if !allow_multiline {
        return Layout::Inline;
    }

    match value {
        Value::Scalar(text) => {
            if render_color_scalar_block_body(text, indent).is_some() {
                Layout::Multiline
            } else {
                Layout::Inline
            }
        }
        Value::Object(object) | Value::AnonymousObject(object) => {
            let _ = object;
            Layout::Multiline
        }
        Value::Array(items) => {
            let _ = items;
            Layout::Multiline
        }
    }
}

fn decide_array_item_layout(value: &Value, indent: usize) -> Layout {
    match value {
        Value::Scalar(text) => {
            if render_split_scalar_list_item(value, "").is_some() {
                return Layout::Multiline;
            }
            if render_color_scalar_block_body(text, indent).is_some() {
                return Layout::Multiline;
            }
            Layout::Inline
        }
        Value::Object(object) | Value::AnonymousObject(object) => {
            let _ = object;
            Layout::Multiline
        }
        Value::Array(items) => {
            let _ = items;
            Layout::Multiline
        }
    }
}

fn render_multiline_entry_value(
    value: &Value,
    indent: usize,
    trim_numeric_scalars: bool,
) -> Result<String, Hoi4ParserError> {
    match value {
        Value::Scalar(text) => {
            render_color_scalar_block_body(text, indent).ok_or_else(|| Hoi4ParserError::Generate {
                message: "标量值未命中多行生成规则".to_string(),
            })
        }
        Value::Object(object) | Value::AnonymousObject(object) => {
            if object.entries().is_empty() {
                Ok(String::new())
            } else {
                render_object_node(object, indent, false, trim_numeric_scalars)
            }
        }
        Value::Array(items) => {
            if items.is_empty() {
                Ok(String::new())
            } else {
                render_multiline_array_block_with_options(items, indent, trim_numeric_scalars)
            }
        }
    }
}

fn wrap_inline_braces(inner: &str) -> String {
    let mut out = String::with_capacity(inner.len() + 4);
    out.push('{');
    out.push(' ');
    out.push_str(inner);
    out.push(' ');
    out.push('}');
    out
}

fn join_key_value(key: &str, value: &str) -> String {
    let mut out = String::with_capacity(key.len() + value.len() + 3);
    out.push_str(key);
    out.push_str(" = ");
    out.push_str(value);
    out
}

fn wrap_keyed_block(key: &str, body: &str, indent: usize) -> String {
    if body.is_empty() {
        return render_empty_keyed_block(key, indent);
    }
    let close_prefix = "\t".repeat(indent);
    let mut out = String::with_capacity(key.len() + body.len() + close_prefix.len() + 8);
    out.push_str(key);
    out.push_str(" = {\n");
    out.push_str(body);
    out.push('\n');
    out.push_str(&close_prefix);
    out.push('}');
    out
}

fn render_empty_keyed_block(key: &str, indent: usize) -> String {
    let close_prefix = "\t".repeat(indent);
    let mut out = String::with_capacity(key.len() + close_prefix.len() + 8);
    out.push_str(key);
    out.push_str(" = {\n");
    out.push_str(&close_prefix);
    out.push('}');
    out
}

fn open_anonymous_block_line(prefix: &str) -> String {
    let mut out = String::with_capacity(prefix.len() + 1);
    out.push_str(prefix);
    out.push('{');
    out
}

fn close_brace_line(prefix: &str) -> String {
    let mut out = String::with_capacity(prefix.len() + 1);
    out.push_str(prefix);
    out.push('}');
    out
}

fn append_line(out: &mut String, line: &str, is_first_line: &mut bool) {
    if *is_first_line {
        *is_first_line = false;
    } else {
        out.push('\n');
    }
    out.push_str(line);
}

fn append_block(out: &mut String, block: &str, is_first_line: &mut bool) {
    if block.is_empty() {
        return;
    }
    if *is_first_line {
        *is_first_line = false;
    } else {
        out.push('\n');
    }
    out.push_str(block);
}

fn concat_with_prefix(prefix: &str, text: &str) -> String {
    let mut out = String::with_capacity(prefix.len() + text.len());
    out.push_str(prefix);
    out.push_str(text);
    out
}

fn render_scalar_for_generate<'a>(
    raw: &'a str,
    trim_numeric_scalars: bool,
) -> Option<Cow<'a, str>> {
    if !trim_numeric_scalars {
        return None;
    }

    let normalized = trim_numeric_scalar_like_java(raw);
    if matches!(normalized, Cow::Borrowed(_)) {
        None
    } else {
        Some(normalized)
    }
}

fn render_entry_scalar_for_generate<'a>(
    key: &str,
    raw: &'a str,
    trim_numeric_scalars: bool,
) -> Cow<'a, str> {
    let template_counter = trim_template_counter_scalar_like_java(key, raw);
    if !matches!(template_counter, Cow::Borrowed(_)) {
        return template_counter;
    }

    render_scalar_for_generate(raw, trim_numeric_scalars).unwrap_or_else(|| Cow::Borrowed(raw))
}

fn trim_numeric_scalar_like_java(raw: &str) -> Cow<'_, str> {
    let decimal = trim_plain_decimal_scalar_like_java(raw);
    if !matches!(decimal, Cow::Borrowed(_)) {
        return decimal;
    }

    trim_operator_numeric_scalar_like_java(raw)
}

fn trim_plain_decimal_scalar_like_java(raw: &str) -> Cow<'_, str> {
    if raw.is_empty()
        || raw.starts_with('"')
        || raw.ends_with('"')
        || raw.contains(char::is_whitespace)
    {
        return Cow::Borrowed(raw);
    }

    let (sign, body) = match raw.as_bytes().first() {
        Some(b'+') | Some(b'-') => (&raw[..1], &raw[1..]),
        _ => ("", raw),
    };

    let Some((integer, fractional)) = body.split_once('.') else {
        return Cow::Borrowed(raw);
    };
    if integer.is_empty()
        || fractional.is_empty()
        || fractional.contains('.')
        || !integer.chars().all(|ch| ch.is_ascii_digit())
        || !fractional.chars().all(|ch| ch.is_ascii_digit())
    {
        return Cow::Borrowed(raw);
    }

    let trimmed_fractional = fractional.trim_end_matches('0');
    if trimmed_fractional.len() == fractional.len() {
        return Cow::Borrowed(raw);
    }

    let mut out = String::with_capacity(raw.len());
    out.push_str(sign);
    out.push_str(integer);
    if !trimmed_fractional.is_empty() {
        out.push('.');
        out.push_str(trimmed_fractional);
    }
    Cow::Owned(out)
}

fn trim_operator_numeric_scalar_like_java(raw: &str) -> Cow<'_, str> {
    if raw.is_empty() || raw.starts_with('"') || raw.ends_with('"') {
        return Cow::Borrowed(raw);
    }

    let (operator, rest) = if let Some(rest) = raw.strip_prefix(">=") {
        (">=", rest)
    } else if let Some(rest) = raw.strip_prefix("<=") {
        ("<=", rest)
    } else if let Some(rest) = raw.strip_prefix("!=") {
        ("!=", rest)
    } else if let Some(rest) = raw.strip_prefix('>') {
        (">", rest)
    } else if let Some(rest) = raw.strip_prefix('<') {
        ("<", rest)
    } else if let Some(rest) = raw.strip_prefix('=') {
        ("=", rest)
    } else {
        return Cow::Borrowed(raw);
    };

    let trimmed_start = rest.trim_start();
    let leading_ws_len = rest.len() - trimmed_start.len();
    let leading_ws = &rest[..leading_ws_len];
    let trimmed_end = trimmed_start.trim_end();
    let trailing_ws = &trimmed_start[trimmed_end.len()..];

    if trimmed_end.is_empty() || trimmed_end.contains(char::is_whitespace) {
        return Cow::Borrowed(raw);
    }

    let numeric = trim_plain_decimal_scalar_like_java(trimmed_end);
    let Cow::Owned(numeric) = numeric else {
        return Cow::Borrowed(raw);
    };

    let mut out = String::with_capacity(raw.len());
    out.push_str(operator);
    out.push_str(leading_ws);
    out.push_str(&numeric);
    out.push_str(trailing_ws);
    Cow::Owned(out)
}

fn trim_template_counter_scalar_like_java<'a>(key: &str, raw: &'a str) -> Cow<'a, str> {
    if key != "template_counter"
        || raw.is_empty()
        || raw.starts_with('"')
        || raw.ends_with('"')
        || raw.contains(char::is_whitespace)
        || !raw.chars().all(|ch| ch.is_ascii_digit())
        || raw == "0"
    {
        return Cow::Borrowed(raw);
    }

    let trimmed = raw.trim_start_matches('0');
    if trimmed.is_empty() {
        return Cow::Owned("0".to_string());
    }
    if trimmed.len() == raw.len() {
        return Cow::Borrowed(raw);
    }

    Cow::Owned(trimmed.to_string())
}

fn generate_nested_payload(
    value: &Value,
    indent: usize,
    trim_numeric_scalars: bool,
) -> Result<String, Hoi4ParserError> {
    match value {
        Value::Object(object) | Value::AnonymousObject(object) => {
            let mut rendered = String::new();
            for (idx, entry) in object.entries().iter().enumerate() {
                if idx > 0 {
                    rendered.push(' ');
                }
                rendered.push_str(&generate_entry_with_options(
                    entry,
                    indent,
                    true,
                    trim_numeric_scalars,
                )?);
            }
            Ok(rendered)
        }
        other => generate_value_with_options(other, indent, true, trim_numeric_scalars),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        generate_document, trim_numeric_scalar_like_java, trim_template_counter_scalar_like_java,
    };
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
    fn should_render_empty_keyed_object_as_multiline_block() {
        let mut root = ObjectNode::default();
        root.push(Entry::new(
            "available",
            Value::Object(ObjectNode::default()),
        ));
        let document = Document::new(Value::Object(root), "");

        let rendered = generate_document(&document).expect("generate should succeed");
        assert_eq!(rendered, "available = {\n}");
    }

    #[test]
    fn should_render_empty_keyed_array_as_multiline_block() {
        let mut root = ObjectNode::default();
        root.push(Entry::new("names", Value::Array(Vec::new())));
        let document = Document::new(Value::Object(root), "");

        let rendered = generate_document(&document).expect("generate should succeed");
        assert_eq!(rendered, "names = {\n}");
    }

    #[test]
    fn should_emit_multiline_array_scalar_as_one_line_per_token_including_curly_quotes() {
        let mut male = ObjectNode::default();
        male.push(Entry::new(
            "names",
            Value::Array(vec![Value::Scalar(format!(
                "Okna Tsahan {}Lha-bzang{} Alexei",
                '\u{201c}', '\u{201d}'
            ))]),
        ));
        let mut kal = ObjectNode::default();
        kal.push(Entry::new("male", Value::Object(male)));
        let mut root = ObjectNode::default();
        root.push(Entry::new("KAL", Value::Object(kal)));
        let document = Document::new(Value::Object(root), "");

        let rendered = generate_document(&document).expect("generate should succeed");
        assert!(
            rendered.contains("\t\tnames = {\n\t\t\tOkna\n"),
            "expected one name per line, got:\n{rendered}"
        );
        assert!(rendered.contains("\t\t\tTsahan\n"));
        assert!(rendered.contains("Lha-bzang"));
        assert!(rendered.contains("\t\t\tAlexei\n"));
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

    #[test]
    fn should_emit_comparison_attached_to_identifier_spaced_single_line() {
        let mut root = ObjectNode::default();
        root.push(Entry::new(
            "trigger",
            Value::Array(vec![
                Value::Scalar("threat".to_string()),
                Value::Scalar(">0.24".to_string()),
            ]),
        ));
        let document = Document::new(Value::Object(root), "");

        let rendered = generate_document(&document).expect("generate should succeed");
        assert!(
            rendered.contains("threat > 0.24"),
            "expected spaced comparison, got:\n{rendered}"
        );
    }

    #[test]
    fn should_trim_trailing_zeroes_for_direct_numeric_scalars() {
        let mut ai_will_do = ObjectNode::default();
        ai_will_do.push(Entry::new("factor", Value::Scalar("1.000".to_string())));
        ai_will_do.push(Entry::new("ratio", Value::Scalar("0.0100".to_string())));
        ai_will_do.push(Entry::new("untouched", Value::Scalar("42".to_string())));
        ai_will_do.push(Entry::new(
            "template_counter",
            Value::Scalar("06".to_string()),
        ));

        let mut root = ObjectNode::default();
        root.push(Entry::new("ai_will_do", Value::Object(ai_will_do)));
        let document = Document::new(Value::Object(root), "");

        let rendered = generate_document(&document).expect("generate should succeed");
        assert!(rendered.contains("factor = 1"));
        assert!(rendered.contains("ratio = 0.01"));
        assert!(rendered.contains("untouched = 42"));
        assert!(rendered.contains("template_counter = 6"));
    }

    #[test]
    fn should_trim_trailing_zeroes_inside_nested_quoted_payload() {
        let mut nested = ObjectNode::default();
        nested.push(Entry::new("factor", Value::Scalar("1.000".to_string())));
        nested.push(Entry::new("ratio", Value::Scalar("0.0100".to_string())));
        nested.push(Entry::new(
            "template_counter",
            Value::Scalar("07".to_string()),
        ));

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
        assert_eq!(
            rendered,
            "effect = \"factor = 1 ratio = 0.01 template_counter = 7\""
        );
    }

    #[test]
    fn should_only_treat_plain_decimal_scalars_as_trim_candidates() {
        assert_eq!(trim_numeric_scalar_like_java("1.000"), "1");
        assert_eq!(trim_numeric_scalar_like_java("-0.5000"), "-0.5");
        assert_eq!(trim_numeric_scalar_like_java("< 0.50"), "< 0.5");
        assert_eq!(trim_numeric_scalar_like_java(">=0.50"), ">=0.5");
        assert_eq!(trim_numeric_scalar_like_java("1"), "1");
        assert_eq!(trim_numeric_scalar_like_java("\"1.000\""), "\"1.000\"");
        assert_eq!(trim_numeric_scalar_like_java("1965.1.1.1"), "1965.1.1.1");
    }

    #[test]
    fn should_trim_template_counter_leading_zeroes_only_for_that_key() {
        assert_eq!(
            trim_template_counter_scalar_like_java("template_counter", "06"),
            "6"
        );
        assert_eq!(
            trim_template_counter_scalar_like_java("template_counter", "002"),
            "2"
        );
        assert_eq!(
            trim_template_counter_scalar_like_java("template_counter", "0"),
            "0"
        );
        assert_eq!(
            trim_template_counter_scalar_like_java("template_counter", "67"),
            "67"
        );
        assert_eq!(trim_template_counter_scalar_like_java("factor", "06"), "06");
    }

    #[test]
    fn should_trim_trailing_zeroes_for_operator_expression_scalars() {
        let mut limit = ObjectNode::default();
        limit.push(Entry::new(
            "has_war_support",
            Value::Scalar("< 0.50".to_string()),
        ));

        let mut root = ObjectNode::default();
        root.push(Entry::new("limit", Value::Object(limit)));
        let document = Document::new(Value::Object(root), "");

        let rendered = generate_document(&document).expect("generate should succeed");
        assert!(rendered.contains("has_war_support = &lt; 0.5"));
    }

    #[test]
    fn should_trim_trailing_zeroes_for_attached_operator_pairs() {
        let mut root = ObjectNode::default();
        root.push(Entry::new(
            "trigger",
            Value::Array(vec![
                Value::Scalar("has_war_support".to_string()),
                Value::Scalar("<0.50".to_string()),
            ]),
        ));
        let document = Document::new(Value::Object(root), "");

        let rendered = generate_document(&document).expect("generate should succeed");
        assert!(rendered.contains("has_war_support < 0.5"));
    }
}
