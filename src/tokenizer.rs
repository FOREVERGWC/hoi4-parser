use crate::Hoi4ParserError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Ident(String),
    StringLiteral(String),
    Equals,
    LBrace,
    RBrace,
    Newline,
}

pub fn tokenize(input: &str) -> Result<Vec<Token>, Hoi4ParserError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0usize;

    while i < chars.len() {
        let ch = chars[i];
        match ch {
            '\u{feff}' | '\r' | ' ' | '\t' => {
                i += 1;
            }
            '\n' => {
                tokens.push(Token::Newline);
                i += 1;
            }
            '#' => {
                i += 1;
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '=' => {
                tokens.push(Token::Equals);
                i += 1;
            }
            '{' => {
                tokens.push(Token::LBrace);
                i += 1;
            }
            '}' => {
                tokens.push(Token::RBrace);
                i += 1;
            }
            '"' => {
                let mut value = String::new();
                value.push(ch);
                i += 1;

                let mut escaped = false;
                let mut closed = false;
                while i < chars.len() {
                    let c = chars[i];
                    value.push(c);
                    i += 1;

                    if escaped {
                        escaped = false;
                        continue;
                    }
                    if c == '\\' {
                        escaped = true;
                        continue;
                    }
                    if c == '"' {
                        closed = true;
                        break;
                    }
                }

                if !closed {
                    return Err(Hoi4ParserError::Parse {
                        message: "字符串未闭合".to_string(),
                    });
                }

                tokens.push(Token::StringLiteral(value));
            }
            _ => {
                let mut value = String::new();
                while i < chars.len() {
                    let c = chars[i];
                    if matches!(c, '\r' | '\n' | ' ' | '\t' | '#' | '=' | '{' | '}') {
                        break;
                    }
                    value.push(c);
                    i += 1;
                }

                if !value.is_empty() {
                    tokens.push(Token::Ident(value));
                } else {
                    i += 1;
                }
            }
        }
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::{tokenize, Token};

    #[test]
    fn should_tokenize_basic_assignment() {
        let tokens = tokenize("tag = CHI").expect("tokenize should succeed");
        assert_eq!(
            tokens,
            vec![
                Token::Ident("tag".to_string()),
                Token::Equals,
                Token::Ident("CHI".to_string())
            ]
        );
    }

    #[test]
    fn should_ignore_comments_but_keep_newline() {
        let tokens =
            tokenize("tag = CHI # comment\nname = \"A\"").expect("tokenize should succeed");
        assert_eq!(
            tokens,
            vec![
                Token::Ident("tag".to_string()),
                Token::Equals,
                Token::Ident("CHI".to_string()),
                Token::Newline,
                Token::Ident("name".to_string()),
                Token::Equals,
                Token::StringLiteral("\"A\"".to_string())
            ]
        );
    }

    #[test]
    fn should_keep_hash_inside_string() {
        let tokens = tokenize("name = \"A#B\" # comment").expect("tokenize should succeed");
        assert_eq!(
            tokens,
            vec![
                Token::Ident("name".to_string()),
                Token::Equals,
                Token::StringLiteral("\"A#B\"".to_string())
            ]
        );
    }

    #[test]
    fn should_fail_for_unclosed_string() {
        let err = tokenize("name = \"A").expect_err("tokenize should fail");
        assert!(err.to_string().contains("字符串未闭合"));
    }

    #[test]
    fn should_ignore_utf8_bom() {
        let tokens = tokenize("\u{feff}tag = CHI").expect("tokenize should succeed");
        assert_eq!(
            tokens,
            vec![
                Token::Ident("tag".to_string()),
                Token::Equals,
                Token::Ident("CHI".to_string())
            ]
        );
    }
}
