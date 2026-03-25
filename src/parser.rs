use std::collections::HashMap;

use crate::compat::normalize_scalar_for_parse;
use crate::nested::decode_nested_quoted;
use crate::tokenizer::Token;
use crate::{Entry, EntryMetadata, Hoi4ParserError, ObjectNode, Value};

pub fn parse_root(tokens: &[Token]) -> Result<Value, Hoi4ParserError> {
    let mut parser = Parser::new(tokens);
    let object = parser.parse_entries_until_rbrace(false)?;
    Ok(Value::Object(object))
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn parse_entries_until_rbrace(
        &mut self,
        stop_on_rbrace: bool,
    ) -> Result<ObjectNode, Hoi4ParserError> {
        let mut object = ObjectNode::default();
        let mut key_counts: HashMap<String, usize> = HashMap::new();
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
                Token::Ident(_) => {
                    let mut entry = self.parse_entry()?;
                    let key = entry.key().to_string();
                    let count = key_counts.entry(key).or_insert(0);
                    if *count > 0 {
                        entry.metadata_mut().duplicate_index = Some(*count);
                        entry.metadata_mut().duplicate_suffix = Some(format!("$${:X}", *count));
                    }
                    *count += 1;
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
            return Err(Hoi4ParserError::Parse {
                message: "花括号未闭合，缺少 '}'".to_string(),
            });
        }

        Ok(object)
    }

    fn parse_entry(&mut self) -> Result<Entry, Hoi4ParserError> {
        let key = self.expect_ident()?;
        self.expect_equals()?;
        self.skip_newlines();
        let mut value = if matches!(self.peek(), Some(Token::LBrace)) {
            self.parse_value()?
        } else {
            self.parse_scalar_sequence()?
        };
        let mut metadata = EntryMetadata::default();

        if let Value::Scalar(raw) = &value {
            if let Some(decoded) = decode_nested_quoted(raw) {
                if let Ok(tokens) = crate::tokenizer::tokenize(&decoded) {
                    if let Ok(parsed_value) = parse_root(&tokens) {
                        value = parsed_value;
                        metadata.nested_quoted = true;
                    }
                }
            }
        }

        Ok(Entry::with_metadata(key, value, metadata))
    }

    fn parse_scalar_sequence(&mut self) -> Result<Value, Hoi4ParserError> {
        let mut parts: Vec<String> = Vec::new();
        while let Some(token) = self.peek() {
            match token {
                Token::Newline | Token::RBrace => break,
                Token::Ident(s) | Token::StringLiteral(s) => {
                    parts.push(normalize_scalar_for_parse(s));
                    self.pos += 1;
                }
                Token::Equals => {
                    if let Some(last) = parts.last_mut() {
                        if last == ">" || last == "<" || last == "!" || last == "=" {
                            last.push('=');
                        } else {
                            parts.push("=".to_string());
                        }
                    } else {
                        parts.push("=".to_string());
                    }
                    self.pos += 1;
                }
                Token::LBrace => {
                    let braced = self.parse_braced_scalar_block()?;
                    parts.push(braced);
                }
            }
        }

        if parts.is_empty() {
            return Err(Hoi4ParserError::Parse {
                message: "值解析失败，缺少标量内容".to_string(),
            });
        }

        Ok(Value::Scalar(parts.join(" ")))
    }

    fn parse_braced_scalar_block(&mut self) -> Result<String, Hoi4ParserError> {
        let mut parts: Vec<String> = Vec::new();
        let mut depth = 0usize;

        while let Some(token) = self.peek() {
            match token {
                Token::LBrace => {
                    depth += 1;
                    parts.push("{".to_string());
                    self.pos += 1;
                }
                Token::RBrace => {
                    if depth == 0 {
                        return Err(Hoi4ParserError::Parse {
                            message: "花括号未闭合，缺少 '{'".to_string(),
                        });
                    }
                    depth -= 1;
                    parts.push("}".to_string());
                    self.pos += 1;
                    if depth == 0 {
                        return Ok(parts.join(" "));
                    }
                }
                Token::Ident(s) | Token::StringLiteral(s) => {
                    parts.push(normalize_scalar_for_parse(s));
                    self.pos += 1;
                }
                Token::Equals => {
                    parts.push("=".to_string());
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
                Ok(Value::Scalar(normalize_scalar_for_parse(s)))
            }
            Some(Token::Ident(s)) => {
                self.pos += 1;
                Ok(Value::Scalar(normalize_scalar_for_parse(s)))
            }
            Some(Token::LBrace) => {
                self.pos += 1;
                let checkpoint = self.pos;
                match self.parse_entries_until_rbrace(true) {
                    Ok(object) => Ok(Value::Object(object)),
                    Err(_) => {
                        self.pos = checkpoint;
                        let array = self.parse_array_until_rbrace()?;
                        Ok(Value::Array(array))
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

    fn expect_ident(&mut self) -> Result<String, Hoi4ParserError> {
        match self.peek() {
            Some(Token::Ident(s)) => {
                self.pos += 1;
                Ok(s.clone())
            }
            Some(token) => Err(Hoi4ParserError::Parse {
                message: format!("期望键名，但遇到: {:?}", token),
            }),
            None => Err(Hoi4ParserError::Parse {
                message: "期望键名，但输入已结束".to_string(),
            }),
        }
    }

    fn expect_equals(&mut self) -> Result<(), Hoi4ParserError> {
        self.skip_newlines();
        match self.peek() {
            Some(Token::Equals) => {
                self.pos += 1;
                Ok(())
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

    fn peek(&self) -> Option<&'a Token> {
        self.tokens.get(self.pos)
    }

    fn parse_array_until_rbrace(&mut self) -> Result<Vec<Value>, Hoi4ParserError> {
        let mut items: Vec<Value> = Vec::new();
        while let Some(token) = self.peek() {
            match token {
                Token::Newline => {
                    self.pos += 1;
                }
                Token::RBrace => {
                    self.pos += 1;
                    return Ok(items);
                }
                Token::LBrace => {
                    let nested = self.parse_value()?;
                    items.push(nested);
                }
                Token::Ident(s) | Token::StringLiteral(s) => {
                    items.push(Value::Scalar(normalize_scalar_for_parse(s)));
                    self.pos += 1;
                }
                Token::Equals => {
                    items.push(Value::Scalar("=".to_string()));
                    self.pos += 1;
                }
            }
        }

        // 容错：到达文件末尾时，允许数组块隐式闭合。
        Ok(items)
    }
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
    fn should_fail_when_missing_rbrace() {
        let input = "country = { tag = CHI";
        let tokens = tokenize(input).expect("tokenize should succeed");
        let err = parse_root(&tokens).expect_err("parse should fail");
        assert!(err.to_string().contains("缺少 '}'"));
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
