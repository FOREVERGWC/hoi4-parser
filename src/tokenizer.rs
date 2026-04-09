use crate::Hoi4ParserError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token<'a> {
    Ident(&'a str),
    StringLiteral(&'a str),
    Equals,
    LBrace,
    RBrace,
    Newline,
}

pub fn tokenize(input: &str) -> Result<Vec<Token<'_>>, Hoi4ParserError> {
    let mut tokens = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0usize;
    const BOM_UTF8: &[u8; 3] = b"\xEF\xBB\xBF";

    while i < bytes.len() {
        match bytes[i] {
            _ if i + BOM_UTF8.len() <= bytes.len() && &bytes[i..i + BOM_UTF8.len()] == BOM_UTF8 => {
                i += BOM_UTF8.len();
            }
            b'\r' | b' ' | b'\t' => {
                i += 1;
            }
            b'\n' => {
                tokens.push(Token::Newline);
                i += 1;
            }
            b'#' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'=' => {
                tokens.push(Token::Equals);
                i += 1;
            }
            b'{' => {
                tokens.push(Token::LBrace);
                i += 1;
            }
            b'}' => {
                tokens.push(Token::RBrace);
                i += 1;
            }
            b'"' => {
                let start = i;
                i += 1;

                let mut escaped = false;
                let mut closed = false;
                while i < bytes.len() {
                    let c = bytes[i];
                    i += 1;

                    if escaped {
                        escaped = false;
                        continue;
                    }
                    if c == b'\\' {
                        escaped = true;
                        continue;
                    }
                    if c == b'"' {
                        closed = true;
                        break;
                    }
                }

                if !closed {
                    return Err(Hoi4ParserError::Parse {
                        message: "字符串未闭合".to_string(),
                    });
                }

                tokens.push(Token::StringLiteral(&input[start..i]));
            }
            _ => {
                let start = i;
                while i < bytes.len() {
                    let c = bytes[i];
                    if matches!(c, b'\r' | b'\n' | b' ' | b'\t' | b'#' | b'=' | b'{' | b'}') {
                        break;
                    }
                    // threat>0.24、a<b 等：在比较符处拆成两个 Ident，便于生成统一为 "a op b" 带空格格式
                    if (c == b'>' || c == b'<')
                        && i > start
                        && matches!(bytes[i - 1], b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'_')
                    {
                        let split_here = match bytes.get(i + 1).copied() {
                            None => false,
                            Some(b'=') => false, // >=、<= 留在同一 token，由外层 '=' 等规则处理
                            Some(nc) => {
                                matches!(nc, b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'.')
                            }
                        };
                        if split_here {
                            break;
                        }
                    }
                    i += 1;
                }

                if i > start {
                    tokens.push(Token::Ident(&input[start..i]));
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
            vec![Token::Ident("tag"), Token::Equals, Token::Ident("CHI")]
        );
    }

    #[test]
    fn should_ignore_comments_but_keep_newline() {
        let tokens =
            tokenize("tag = CHI # comment\nname = \"A\"").expect("tokenize should succeed");
        assert_eq!(
            tokens,
            vec![
                Token::Ident("tag"),
                Token::Equals,
                Token::Ident("CHI"),
                Token::Newline,
                Token::Ident("name"),
                Token::Equals,
                Token::StringLiteral("\"A\"")
            ]
        );
    }

    #[test]
    fn should_keep_hash_inside_string() {
        let tokens = tokenize("name = \"A#B\" # comment").expect("tokenize should succeed");
        assert_eq!(
            tokens,
            vec![
                Token::Ident("name"),
                Token::Equals,
                Token::StringLiteral("\"A#B\"")
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
            vec![Token::Ident("tag"), Token::Equals, Token::Ident("CHI")]
        );
    }

    #[test]
    fn should_split_identifier_followed_by_comparison_then_operand() {
        let tokens = tokenize("threat>0.24").expect("tokenize should succeed");
        assert_eq!(tokens, vec![Token::Ident("threat"), Token::Ident(">0.24")]);
    }
}
