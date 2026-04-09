use std::collections::HashMap;

use crate::compat::normalize_scalar_for_parse;
use crate::nested::decode_nested_quoted;
use crate::tokenizer::Token;
use crate::{Entry, EntryMetadata, Hoi4ParserError, ObjectNode, Value};

pub fn parse_root(tokens: &[Token<'_>]) -> Result<Value, Hoi4ParserError> {
    let mut parser = Parser::new(tokens);
    let object = parser.parse_entries_until_rbrace(false)?;
    Ok(Value::Object(object))
}

struct Parser<'a> {
    tokens: &'a [Token<'a>],
    pos: usize,
}

enum ScopeKeyCounter {
    Small(Vec<(String, usize)>),
    Large(HashMap<String, usize>),
}

impl Default for ScopeKeyCounter {
    fn default() -> Self {
        Self::Small(Vec::with_capacity(8))
    }
}

impl ScopeKeyCounter {
    const SMALL_TO_LARGE_THRESHOLD: usize = 12;

    fn next_index(&mut self, key: &str) -> usize {
        match self {
            ScopeKeyCounter::Small(items) => {
                if let Some((_, count)) = items.iter_mut().find(|(existing, _)| existing == key) {
                    let current = *count;
                    *count += 1;
                    return current;
                }

                if items.len() >= Self::SMALL_TO_LARGE_THRESHOLD {
                    let mut map = HashMap::with_capacity(items.len() * 2);
                    for (k, v) in items.drain(..) {
                        map.insert(k, v);
                    }
                    let current = map.get(key).copied().unwrap_or(0);
                    map.insert(key.to_string(), current + 1);
                    *self = ScopeKeyCounter::Large(map);
                    return current;
                }

                items.push((key.to_string(), 1));
                0
            }
            ScopeKeyCounter::Large(map) => {
                let current = map.get(key).copied().unwrap_or(0);
                map.insert(key.to_string(), current + 1);
                current
            }
        }
    }
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token<'a>]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn parse_entries_until_rbrace(
        &mut self,
        stop_on_rbrace: bool,
    ) -> Result<ObjectNode, Hoi4ParserError> {
        let mut object = ObjectNode::default();
        let mut key_counts = ScopeKeyCounter::default();
        while let Some(token) = self.peek() {
            match token {
                Token::Newline => {
                    self.pos += 1;
                }
                Token::RBrace if stop_on_rbrace => {
                    self.pos += 1;
                    return Ok(object);
                }
                Token::RBrace => {
                    // 容错：根级别遇到多余右花括号时忽略，兼容部分游戏原文件。
                    self.pos += 1;
                }
                Token::Ident(_) | Token::StringLiteral(_) => {
                    let mut entry = self.parse_entry()?;
                    let dup_index = key_counts.next_index(entry.key());
                    if dup_index > 0 {
                        entry.metadata_mut().duplicate_index = Some(dup_index);
                        entry.metadata_mut().duplicate_suffix = Some(format!("$${:X}", dup_index));
                    }
                    object.push(entry);
                }
                _ => {
                    return Err(Hoi4ParserError::Parse {
                        message: format!("不期望的 token: {:?}", token),
                    });
                }
            }
        }

        if stop_on_rbrace {
            // 容错：对象块到达文件末尾时，允许隐式闭合，兼容游戏原文件中的不完整花括号。
            return Ok(object);
        }

        Ok(object)
    }

    fn parse_entry(&mut self) -> Result<Entry, Hoi4ParserError> {
        let key = self.expect_key()?;
        let leading_operator = self.expect_equals_or_operator()?;
        self.skip_newlines();
        let mut value = if matches!(self.peek(), Some(Token::LBrace)) {
            self.parse_value()?
        } else {
            self.parse_scalar_sequence(leading_operator)?
        };
        let mut metadata = EntryMetadata::default();

        if let Value::Scalar(raw) = &value {
            if should_attempt_nested_quoted_parse(key.as_str(), raw) {
                if let Some(decoded) = decode_nested_quoted(raw) {
                    if let Ok(tokens) = crate::tokenizer::tokenize(&decoded) {
                        if let Ok(parsed_value) = parse_root(&tokens) {
                            value = parsed_value;
                            metadata.nested_quoted = true;
                        }
                    }
                }
            }
        }

        Ok(Entry::with_metadata(key, value, metadata))
    }

    fn parse_scalar_sequence(
        &mut self,
        leading_operator: Option<String>,
    ) -> Result<Value, Hoi4ParserError> {
        let mut scalar = String::new();
        let mut has_parts = false;
        let mut last_token_can_merge_equals = false;
        if let Some(op) = leading_operator {
            append_token_with_space(&mut scalar, &mut has_parts, &op);
            last_token_can_merge_equals = matches!(op.as_str(), ">" | "<" | "!" | "=");
        }
        while let Some(token) = self.peek() {
            match token {
                Token::Newline | Token::RBrace => break,
                Token::Ident(s) | Token::StringLiteral(s) => {
                    if has_parts
                        && (self.next_is_key_value_boundary()
                            || self.next_is_implicit_operator_boundary())
                    {
                        break;
                    }
                    let normalized = normalize_scalar_for_parse(s);
                    append_token_with_space(&mut scalar, &mut has_parts, &normalized);
                    last_token_can_merge_equals =
                        matches!(normalized.as_ref(), ">" | "<" | "!" | "=");
                    self.pos += 1;
                }
                Token::Equals => {
                    if has_parts && last_token_can_merge_equals {
                        scalar.push('=');
                    } else {
                        append_token_with_space(&mut scalar, &mut has_parts, "=");
                    }
                    last_token_can_merge_equals = true;
                    self.pos += 1;
                }
                Token::LBrace => {
                    let braced = self.parse_braced_scalar_block()?;
                    append_token_with_space(&mut scalar, &mut has_parts, &braced);
                    last_token_can_merge_equals = false;
                }
            }
        }

        if !has_parts {
            return Err(Hoi4ParserError::Parse {
                message: "值解析失败，缺少标量内容".to_string(),
            });
        }

        Ok(Value::Scalar(scalar))
    }

    fn next_is_key_value_boundary(&self) -> bool {
        matches!(self.peek(), Some(Token::Ident(_)))
            && matches!(self.tokens.get(self.pos + 1), Some(Token::Equals))
    }

    fn next_is_implicit_operator_boundary(&self) -> bool {
        matches!(self.peek(), Some(Token::Ident(_)))
            && matches!(
                self.tokens.get(self.pos + 1),
                Some(Token::Ident(op))
                    if matches!(*op, ">" | "<" | "!" | "=")
            )
    }

    fn parse_braced_scalar_block(&mut self) -> Result<String, Hoi4ParserError> {
        let mut out = String::new();
        let mut has_parts = false;
        let mut depth = 0usize;

        while let Some(token) = self.peek() {
            match token {
                Token::LBrace => {
                    depth += 1;
                    append_token_with_space(&mut out, &mut has_parts, "{");
                    self.pos += 1;
                }
                Token::RBrace => {
                    if depth == 0 {
                        return Err(Hoi4ParserError::Parse {
                            message: "花括号未闭合，缺少 '{'".to_string(),
                        });
                    }
                    depth -= 1;
                    append_token_with_space(&mut out, &mut has_parts, "}");
                    self.pos += 1;
                    if depth == 0 {
                        return Ok(out);
                    }
                }
                Token::Ident(s) | Token::StringLiteral(s) => {
                    let normalized = normalize_scalar_for_parse(s);
                    append_token_with_space(&mut out, &mut has_parts, &normalized);
                    self.pos += 1;
                }
                Token::Equals => {
                    append_token_with_space(&mut out, &mut has_parts, "=");
                    self.pos += 1;
                }
                Token::Newline => {
                    self.pos += 1;
                }
            }
        }

        Err(Hoi4ParserError::Parse {
            message: "标量中的花括号块未闭合，缺少 '}'".to_string(),
        })
    }

    fn parse_value(&mut self) -> Result<Value, Hoi4ParserError> {
        match self.peek() {
            Some(Token::StringLiteral(s)) => {
                self.pos += 1;
                Ok(Value::Scalar(normalize_scalar_for_parse(s).into_owned()))
            }
            Some(Token::Ident(s)) => {
                self.pos += 1;
                Ok(Value::Scalar(normalize_scalar_for_parse(s).into_owned()))
            }
            Some(Token::LBrace) => {
                self.pos += 1;
                let checkpoint = self.pos;
                match self.classify_braced_value() {
                    BracedValueKind::Object => match self.parse_entries_until_rbrace(true) {
                        Ok(object) => Ok(Value::Object(object)),
                        Err(_) => {
                            // 兼容历史容错：对象判型失败时回退数组解析。
                            self.pos = checkpoint;
                            let array = self.parse_array_until_rbrace()?;
                            if let Some(color_scalar) = collapse_color_array_to_scalar(&array) {
                                Ok(Value::Scalar(color_scalar))
                            } else {
                                Ok(Value::Array(array))
                            }
                        }
                    },
                    BracedValueKind::Array => {
                        let array = self.parse_array_until_rbrace()?;
                        if let Some(color_scalar) = collapse_color_array_to_scalar(&array) {
                            Ok(Value::Scalar(color_scalar))
                        } else {
                            Ok(Value::Array(array))
                        }
                    }
                }
            }
            Some(token) => Err(Hoi4ParserError::Parse {
                message: format!("值解析失败，遇到不期望的 token: {:?}", token),
            }),
            None => Err(Hoi4ParserError::Parse {
                message: "值解析失败，输入提前结束".to_string(),
            }),
        }
    }

    fn expect_key(&mut self) -> Result<String, Hoi4ParserError> {
        match self.peek() {
            Some(Token::Ident(s)) => {
                self.pos += 1;
                Ok(s.to_string())
            }
            Some(Token::StringLiteral(s)) => {
                self.pos += 1;
                Ok(normalize_scalar_for_parse(s).into_owned())
            }
            Some(token) => Err(Hoi4ParserError::Parse {
                message: format!("期望键名，但遇到: {:?}", token),
            }),
            None => Err(Hoi4ParserError::Parse {
                message: "期望键名，但输入已结束".to_string(),
            }),
        }
    }

    fn expect_equals_or_operator(&mut self) -> Result<Option<String>, Hoi4ParserError> {
        self.skip_newlines();
        match self.peek() {
            Some(Token::Equals) => {
                self.pos += 1;
                Ok(None)
            }
            Some(Token::Ident(op)) if matches!(*op, ">" | "<" | "!" | "=") => {
                let mut operator = op.to_string();
                self.pos += 1;
                if matches!(self.peek(), Some(Token::Equals)) {
                    self.pos += 1;
                    operator.push('=');
                }
                Ok(Some(operator))
            }
            Some(token) => Err(Hoi4ParserError::Parse {
                message: format!("期望 '='，但遇到: {:?}", token),
            }),
            None => Err(Hoi4ParserError::Parse {
                message: "期望 '='，但输入已结束".to_string(),
            }),
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek(), Some(Token::Newline)) {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<&'a Token<'a>> {
        self.tokens.get(self.pos)
    }

    fn parse_array_until_rbrace(&mut self) -> Result<Vec<Value>, Hoi4ParserError> {
        let mut events: Vec<ArrayEvent> = Vec::new();
        let mut has_top_level_newline = false;
        while let Some(token) = self.peek() {
            match token {
                Token::Newline => {
                    has_top_level_newline = true;
                    events.push(ArrayEvent::Newline);
                    self.pos += 1;
                }
                Token::RBrace => {
                    self.pos += 1;
                    let items = if has_top_level_newline {
                        build_multiline_array_items(events)
                    } else {
                        build_singleline_array_items(events)
                    };
                    return Ok(merge_adjacent_split_quoted_scalars(items));
                }
                Token::LBrace => {
                    let nested = self.parse_array_item_value()?;
                    events.push(ArrayEvent::Nested(nested));
                }
                Token::Ident(s) | Token::StringLiteral(s) => {
                    events.push(ArrayEvent::Scalar(
                        normalize_scalar_for_parse(s).into_owned(),
                    ));
                    self.pos += 1;
                }
                Token::Equals => {
                    events.push(ArrayEvent::Equals);
                    self.pos += 1;
                }
            }
        }

        // 容错：到达文件末尾时，允许数组块隐式闭合。
        let items = if has_top_level_newline {
            build_multiline_array_items(events)
        } else {
            build_singleline_array_items(events)
        };
        Ok(merge_adjacent_split_quoted_scalars(items))
    }

    fn classify_braced_value(&self) -> BracedValueKind {
        let mut idx = self.pos;

        while let Some(Token::Newline) = self.tokens.get(idx) {
            idx += 1;
        }

        match self.tokens.get(idx) {
            Some(Token::RBrace) => BracedValueKind::Object,
            Some(Token::LBrace) | Some(Token::Equals) => BracedValueKind::Array,
            Some(Token::Ident(_)) | Some(Token::StringLiteral(_)) => {
                idx += 1;
                while let Some(Token::Newline) = self.tokens.get(idx) {
                    idx += 1;
                }
                match self.tokens.get(idx) {
                    Some(Token::Equals) => BracedValueKind::Object,
                    Some(Token::Ident(op)) if matches!(*op, ">" | "<" | "!" | "=") => {
                        BracedValueKind::Object
                    }
                    _ => BracedValueKind::Array,
                }
            }
            _ => BracedValueKind::Array,
        }
    }

    fn parse_array_item_value(&mut self) -> Result<Value, Hoi4ParserError> {
        match self.peek() {
            Some(Token::LBrace) => {
                self.pos += 1;
                let checkpoint = self.pos;
                match self.classify_braced_value() {
                    BracedValueKind::Object => match self.parse_entries_until_rbrace(true) {
                        Ok(object) => Ok(Value::AnonymousObject(object)),
                        Err(_) => {
                            self.pos = checkpoint;
                            let array = self.parse_array_until_rbrace()?;
                            if let Some(color_scalar) = collapse_color_array_to_scalar(&array) {
                                Ok(Value::Scalar(color_scalar))
                            } else {
                                Ok(Value::Array(array))
                            }
                        }
                    },
                    BracedValueKind::Array => {
                        let array = self.parse_array_until_rbrace()?;
                        if let Some(color_scalar) = collapse_color_array_to_scalar(&array) {
                            Ok(Value::Scalar(color_scalar))
                        } else {
                            Ok(Value::Array(array))
                        }
                    }
                }
            }
            _ => self.parse_value(),
        }
    }
}

fn append_token_with_space(out: &mut String, has_parts: &mut bool, token: &str) {
    if *has_parts {
        out.push(' ');
    } else {
        *has_parts = true;
    }
    out.push_str(token);
}

fn should_attempt_nested_quoted_parse(key: &str, raw: &str) -> bool {
    // 优先匹配常见脚本容器；同时兼容 division 等把脚本压成 quoted scalar 的字段。
    let key_likely_nested = matches!(key, "effect" | "trigger" | "modifier" | "division")
        || key.ends_with("_effect")
        || key.ends_with("_trigger")
        || key.ends_with("_modifier");
    let value_likely_nested = raw.starts_with('"')
        && raw.ends_with('"')
        && raw.contains("\\\"")
        && raw.contains('=')
        && (raw.contains('{') || raw.contains('}') || raw.matches('=').count() >= 2);
    key_likely_nested || value_likely_nested
}

enum BracedValueKind {
    Object,
    Array,
}

enum ArrayEvent {
    Scalar(String),
    Equals,
    Nested(Value),
    Newline,
}

fn build_singleline_array_items(events: Vec<ArrayEvent>) -> Vec<Value> {
    let mut items = Vec::with_capacity(events.len());
    for event in events {
        match event {
            ArrayEvent::Scalar(s) => items.push(Value::Scalar(s)),
            ArrayEvent::Equals => items.push(Value::Scalar("=".to_string())),
            ArrayEvent::Nested(v) => items.push(v),
            ArrayEvent::Newline => {}
        }
    }
    items
}

fn build_multiline_array_items(events: Vec<ArrayEvent>) -> Vec<Value> {
    let mut items: Vec<Value> = Vec::new();
    let mut line_parts: Vec<String> = Vec::new();
    for event in events {
        match event {
            ArrayEvent::Newline => {
                // 弯引号 “…” / ‘…’ 或 ASCII "…" 未闭合时不要把换行当分段，否则 “Sapa / Inca” 会变成两个标量
                if !line_parts.is_empty() && !joined_line_has_unclosed_quote_like(&line_parts) {
                    items.push(Value::Scalar(join_parts_with_space(&line_parts)));
                    line_parts.clear();
                }
            }
            ArrayEvent::Scalar(s) => line_parts.push(s),
            ArrayEvent::Equals => push_or_merge_equals(&mut line_parts),
            ArrayEvent::Nested(v) => {
                if !line_parts.is_empty() {
                    items.push(Value::Scalar(join_parts_with_space(&line_parts)));
                    line_parts.clear();
                }
                items.push(v);
            }
        }
    }
    if !line_parts.is_empty() {
        items.push(Value::Scalar(join_parts_with_space(&line_parts)));
    }
    items
}

/// 单行内 tokenizer 会在空白处断开，`“Sapa` 与 `Inca”` 会变成两个标量；若前段引号未闭合则与后段合并。
fn merge_adjacent_split_quoted_scalars(items: Vec<Value>) -> Vec<Value> {
    let mut merged: Vec<Value> = Vec::with_capacity(items.len());
    for item in items {
        if let Value::Scalar(next_text) = &item {
            if let Some(Value::Scalar(last_text)) = merged.last_mut() {
                if line_has_unclosed_quote_like(last_text) {
                    last_text.push(' ');
                    last_text.push_str(next_text);
                    continue;
                }
            }
        }
        merged.push(item);
    }
    merged
}

fn joined_line_has_unclosed_quote_like(parts: &[String]) -> bool {
    line_has_unclosed_quote_like_parts(parts)
}

/// 扫描 `s`：若末尾仍处于未闭合的 ASCII `"` 或弯引号 `“…”` / `‘…’` 内，返回 true。
fn line_has_unclosed_quote_like(s: &str) -> bool {
    let mut in_ascii = false;
    let mut escaped = false;
    let mut in_curly = false;

    for ch in s.chars() {
        if in_ascii {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '"' {
                in_ascii = false;
            }
            continue;
        }
        if in_curly {
            if matches!(ch, '\u{201d}' | '\u{2019}') {
                in_curly = false;
            }
            continue;
        }
        if ch == '"' {
            in_ascii = true;
        } else if matches!(ch, '\u{201c}' | '\u{2018}') {
            in_curly = true;
        }
    }

    in_ascii || in_curly
}

fn collapse_color_array_to_scalar(items: &[Value]) -> Option<String> {
    if items.len() < 2 {
        return None;
    }
    let Value::Scalar(first) = &items[0] else {
        return None;
    };
    if first != "rgb" && first != "HSV" && first != "hsv" {
        return None;
    }
    let mut out = String::with_capacity(first.len() + items.len() * 4);
    out.push_str(first);
    out.push_str(" { ");
    let mut wrote_any = false;
    for item in items.iter().skip(1) {
        let Value::Scalar(text) = item else {
            return None;
        };
        if text.contains('\n') {
            return None;
        }
        if wrote_any {
            out.push(' ');
        } else {
            wrote_any = true;
        }
        out.push_str(text);
    }
    out.push_str(" }");
    Some(out)
}

fn join_parts_with_space(parts: &[String]) -> String {
    if parts.is_empty() {
        return String::new();
    }
    let mut total = parts.len() - 1;
    for part in parts {
        total += part.len();
    }
    let mut out = String::with_capacity(total);
    for (idx, part) in parts.iter().enumerate() {
        if idx > 0 {
            out.push(' ');
        }
        out.push_str(part);
    }
    out
}

fn line_has_unclosed_quote_like_parts(parts: &[String]) -> bool {
    let mut in_ascii = false;
    let mut escaped = false;
    let mut in_curly = false;

    for (idx, part) in parts.iter().enumerate() {
        if idx > 0 {
            scan_quote_state_char(' ', &mut in_ascii, &mut escaped, &mut in_curly);
        }
        for ch in part.chars() {
            scan_quote_state_char(ch, &mut in_ascii, &mut escaped, &mut in_curly);
        }
    }

    in_ascii || in_curly
}

fn scan_quote_state_char(ch: char, in_ascii: &mut bool, escaped: &mut bool, in_curly: &mut bool) {
    if *in_ascii {
        if *escaped {
            *escaped = false;
            return;
        }
        if ch == '\\' {
            *escaped = true;
            return;
        }
        if ch == '"' {
            *in_ascii = false;
        }
        return;
    }
    if *in_curly {
        if matches!(ch, '\u{201d}' | '\u{2019}') {
            *in_curly = false;
        }
        return;
    }
    if ch == '"' {
        *in_ascii = true;
    } else if matches!(ch, '\u{201c}' | '\u{2018}') {
        *in_curly = true;
    }
}

fn push_or_merge_equals(parts: &mut Vec<String>) {
    if let Some(last) = parts.last_mut() {
        if matches!(last.as_str(), ">" | "<" | "!" | "=") {
            last.push('=');
            return;
        }
    }
    parts.push("=".to_string());
}

#[cfg(test)]
mod tests {
    use super::parse_root;
    use crate::tokenizer::tokenize;
    use crate::{compat::export_key, Value};

    #[test]
    fn should_parse_nested_object() {
        let input = "country = { tag = CHI name = \"China\" }";
        let tokens = tokenize(input).expect("tokenize should succeed");
        let root = parse_root(&tokens).expect("parse should succeed");
        match root {
            Value::Object(root_object) => {
                assert_eq!(root_object.entries().len(), 1);
                assert_eq!(root_object.entries()[0].key(), "country");
            }
            _ => panic!("root should be object"),
        }
    }

    #[test]
    fn should_tolerate_missing_rbrace_for_object_at_eof() {
        let input = "country = { tag = CHI";
        let tokens = tokenize(input).expect("tokenize should succeed");
        let root = parse_root(&tokens).expect("parse should succeed");
        let Value::Object(root_object) = root else {
            panic!("root should be object");
        };
        assert_eq!(root_object.entries().len(), 1);
        assert_eq!(root_object.entries()[0].key(), "country");
        assert!(matches!(
            root_object.entries()[0].value(),
            Value::Object(object) if object.entries().len() == 1
        ));
    }

    #[test]
    fn should_record_duplicate_metadata_for_same_scope() {
        let input = "name = alpha\nname = beta";
        let tokens = tokenize(input).expect("tokenize should succeed");
        let root = parse_root(&tokens).expect("parse should succeed");
        let Value::Object(root_object) = root else {
            panic!("root should be object");
        };

        assert_eq!(root_object.entries().len(), 2);
        let first = &root_object.entries()[0];
        let second = &root_object.entries()[1];

        assert_eq!(first.metadata().duplicate_index, None);
        assert_eq!(second.metadata().duplicate_index, Some(1));
        assert_eq!(export_key(second, true), "name$$1");
    }

    #[test]
    fn should_parse_nested_quoted_object_and_mark_metadata() {
        let input = "effect = \"set_variable = { name = \\\"x\\\" value = 1 }\"";
        let tokens = tokenize(input).expect("tokenize should succeed");
        let root = parse_root(&tokens).expect("parse should succeed");
        let Value::Object(root_object) = root else {
            panic!("root should be object");
        };

        let effect_entry = &root_object.entries()[0];
        assert!(effect_entry.metadata().nested_quoted);
        assert!(matches!(effect_entry.value(), Value::Object(_)));
    }

    #[test]
    fn should_parse_nested_quoted_division_payload_and_mark_metadata() {
        let input =
            "division = \"name = \\\"Sikh Division\\\" division_template = \\\"Sikh Division\\\"\"";
        let tokens = tokenize(input).expect("tokenize should succeed");
        let root = parse_root(&tokens).expect("parse should succeed");
        let Value::Object(root_object) = root else {
            panic!("root should be object");
        };

        let division_entry = &root_object.entries()[0];
        assert!(division_entry.metadata().nested_quoted);
        let Value::Object(division_object) = division_entry.value() else {
            panic!("division payload should be parsed as object");
        };
        assert_eq!(division_object.entries().len(), 2);
        assert!(matches!(
            division_object.entries()[0].value(),
            Value::Scalar(v) if v == "\"Sikh Division\""
        ));
        assert!(matches!(
            division_object.entries()[1].value(),
            Value::Scalar(v) if v == "\"Sikh Division\""
        ));
    }

    #[test]
    fn should_parse_object_with_string_literal_key() {
        let input = "air_wings = { \"USS Enterprise\" = { version_name = \"F3F\" } }";
        let tokens = tokenize(input).expect("tokenize should succeed");
        let root = parse_root(&tokens).expect("parse should succeed");
        let Value::Object(root_object) = root else {
            panic!("root should be object");
        };
        let Value::Object(air_wings) = root_object.entries()[0].value() else {
            panic!("air_wings should be object");
        };
        assert_eq!(air_wings.entries().len(), 1);
        assert_eq!(air_wings.entries()[0].key(), "\"USS Enterprise\"");
        assert!(matches!(air_wings.entries()[0].value(), Value::Object(_)));
    }

    #[test]
    fn should_parse_operator_expression_as_scalar_sequence() {
        let input = "trigger = >= 0.35";
        let tokens = tokenize(input).expect("tokenize should succeed");
        let root = parse_root(&tokens).expect("parse should succeed");
        let Value::Object(root_object) = root else {
            panic!("root should be object");
        };
        assert!(matches!(
            root_object.entries()[0].value(),
            Value::Scalar(v) if v == ">= 0.35"
        ));
    }

    #[test]
    fn should_parse_array_style_block() {
        let input = "names = { \"A\" \"B\" \"C\" }";
        let tokens = tokenize(input).expect("tokenize should succeed");
        let root = parse_root(&tokens).expect("parse should succeed");
        let Value::Object(root_object) = root else {
            panic!("root should be object");
        };
        assert!(matches!(
            root_object.entries()[0].value(),
            Value::Array(items) if items.len() == 3
        ));
    }

    #[test]
    fn should_parse_anonymous_object_inside_array_style_block() {
        let input = "optional_assets = { { icon = \"GFX_ship\" model = \"ship_entity\" } }";
        let tokens = tokenize(input).expect("tokenize should succeed");
        let root = parse_root(&tokens).expect("parse should succeed");
        let Value::Object(root_object) = root else {
            panic!("root should be object");
        };
        let Value::Array(items) = root_object.entries()[0].value() else {
            panic!("optional_assets should be array");
        };
        assert_eq!(items.len(), 1);
        let Value::AnonymousObject(asset) = &items[0] else {
            panic!("array item should be anonymous object");
        };
        assert_eq!(asset.entries().len(), 2);
        assert_eq!(asset.entries()[0].key(), "icon");
        assert_eq!(asset.entries()[1].key(), "model");
    }

    #[test]
    fn should_keep_curly_quoted_phrase_when_multiline_array_splits_across_newline() {
        let input = "callsigns = {\n\t\t“Sapa\n\t\tInca”\n\t\tAymara\n\t}";
        let tokens = tokenize(input).expect("tokenize should succeed");
        let root = parse_root(&tokens).expect("parse should succeed");
        let Value::Object(root_object) = root else {
            panic!("root should be object");
        };
        let Value::Array(items) = root_object.entries()[0].value() else {
            panic!("expected array");
        };
        assert_eq!(items.len(), 2, "expected one scalar for Sapa Inca + Aymara");
        let Value::Scalar(s0) = &items[0] else {
            panic!("first item scalar");
        };
        assert_eq!(s0, "“Sapa Inca”");
        let Value::Scalar(s1) = &items[1] else {
            panic!("second item scalar");
        };
        assert_eq!(s1, "Aymara");
    }

    #[test]
    fn should_merge_curly_quoted_phrase_split_by_space_on_one_line() {
        // tokenizer 在空格处断开，无换行时走单行数组解析，须合并相邻标量
        let input = "callsigns = { “Sapa Inca” Aymara }";
        let tokens = tokenize(input).expect("tokenize should succeed");
        let root = parse_root(&tokens).expect("parse should succeed");
        let Value::Object(root_object) = root else {
            panic!("root should be object");
        };
        let Value::Array(items) = root_object.entries()[0].value() else {
            panic!("expected array");
        };
        assert_eq!(items.len(), 2);
        let Value::Scalar(s0) = &items[0] else {
            panic!("first scalar");
        };
        assert_eq!(s0, "“Sapa Inca”");
        assert!(matches!(&items[1], Value::Scalar(s) if s == "Aymara"));
    }

    #[test]
    fn should_parse_rgb_block_as_scalar_sequence() {
        let input = "color = rgb { 153 0 51 }";
        let tokens = tokenize(input).expect("tokenize should succeed");
        let root = parse_root(&tokens).expect("parse should succeed");
        let Value::Object(root_object) = root else {
            panic!("root should be object");
        };
        assert!(matches!(
            root_object.entries()[0].value(),
            Value::Scalar(v) if v == "rgb { 153 0 51 }"
        ));
    }

    #[test]
    fn should_parse_braced_rgb_block_as_scalar_sequence() {
        let input = "color = { rgb 153 0 51 }";
        let tokens = tokenize(input).expect("tokenize should succeed");
        let root = parse_root(&tokens).expect("parse should succeed");
        let Value::Object(root_object) = root else {
            panic!("root should be object");
        };
        assert!(matches!(
            root_object.entries()[0].value(),
            Value::Scalar(v) if v == "rgb { 153 0 51 }"
        ));
    }

    #[test]
    fn should_parse_braced_hsv_block_as_scalar_sequence() {
        let input = "color = { HSV 0.1 0.15 0.4 }";
        let tokens = tokenize(input).expect("tokenize should succeed");
        let root = parse_root(&tokens).expect("parse should succeed");
        let Value::Object(root_object) = root else {
            panic!("root should be object");
        };
        assert!(matches!(
            root_object.entries()[0].value(),
            Value::Scalar(v) if v == "HSV { 0.1 0.15 0.4 }"
        ));
    }

    #[test]
    fn should_allow_newline_between_key_and_equals() {
        let input = "foo\n= bar";
        let tokens = tokenize(input).expect("tokenize should succeed");
        let root = parse_root(&tokens).expect("parse should succeed");
        let Value::Object(root_object) = root else {
            panic!("root should be object");
        };
        assert!(matches!(
            root_object.entries()[0].value(),
            Value::Scalar(v) if v == "bar"
        ));
    }

    #[test]
    fn should_allow_newline_between_equals_and_object_value() {
        let input = "foo =\n{ bar = baz }";
        let tokens = tokenize(input).expect("tokenize should succeed");
        let root = parse_root(&tokens).expect("parse should succeed");
        let Value::Object(root_object) = root else {
            panic!("root should be object");
        };
        assert!(matches!(root_object.entries()[0].value(), Value::Object(_)));
    }

    #[test]
    fn should_ignore_extra_rbrace_at_root() {
        let input = "foo = bar\n}";
        let tokens = tokenize(input).expect("tokenize should succeed");
        let root = parse_root(&tokens).expect("parse should succeed");
        let Value::Object(root_object) = root else {
            panic!("root should be object");
        };
        assert_eq!(root_object.entries().len(), 1);
        assert_eq!(root_object.entries()[0].key(), "foo");
    }

    #[test]
    fn should_tolerate_unclosed_array_at_eof() {
        let input = "items = { 1 2 3";
        let tokens = tokenize(input).expect("tokenize should succeed");
        let root = parse_root(&tokens).expect("parse should succeed");
        let Value::Object(root_object) = root else {
            panic!("root should be object");
        };
        assert!(matches!(
            root_object.entries()[0].value(),
            Value::Array(items) if items.len() == 3
        ));
    }

    #[test]
    fn duplicate_key_count_should_be_isolated_per_scope() {
        let input = "country = { name = A name = B }\nname = C";
        let tokens = tokenize(input).expect("tokenize should succeed");
        let root = parse_root(&tokens).expect("parse should succeed");
        let Value::Object(root_object) = root else {
            panic!("root should be object");
        };

        assert_eq!(root_object.entries().len(), 2);
        assert_eq!(root_object.entries()[1].key(), "name");
        assert_eq!(root_object.entries()[1].metadata().duplicate_index, None);
    }
}
