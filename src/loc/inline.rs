//! Paradox 本地化 yml 内联语法解析。
//!
//! value 字符串内部支持：
//!   §X..§!        颜色：push 颜色 X / pop 上一层
//!   $key$         引用另一条 loc
//!   [Scope.Get..] 运行时取值（前端无法求值，整体保留 raw）
//!   £icon£        图标，配对版
//!   £icon         图标，至下一个空白/特殊符号自终止
//!   \n \t         换行/制表
//!
//! 解析永远成功：未闭合的 $/[/£ 会退化为 Text，避免吞掉文件。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InlineNode {
    Text { value: String },
    Ref { key: String },
    ColorPush { code: char },
    ColorPop,
    Scope { expr: String },
    Icon { token: String },
    LineBreak,
    Tab,
}

pub fn parse_inline(input: &str) -> Vec<InlineNode> {
    let chars: Vec<char> = input.chars().collect();
    let mut out: Vec<InlineNode> = Vec::new();
    let mut text = String::new();
    let mut i = 0usize;

    while i < chars.len() {
        let c = chars[i];
        match c {
            '\\' if i + 1 < chars.len() => match chars[i + 1] {
                'n' => {
                    flush(&mut text, &mut out);
                    out.push(InlineNode::LineBreak);
                    i += 2;
                }
                't' => {
                    flush(&mut text, &mut out);
                    out.push(InlineNode::Tab);
                    i += 2;
                }
                '"' => {
                    text.push('"');
                    i += 2;
                }
                '\\' => {
                    text.push('\\');
                    i += 2;
                }
                other => {
                    text.push('\\');
                    text.push(other);
                    i += 2;
                }
            },
            '§' => {
                if i + 1 < chars.len() {
                    if chars[i + 1] == '!' {
                        flush(&mut text, &mut out);
                        out.push(InlineNode::ColorPop);
                        i += 2;
                    } else {
                        flush(&mut text, &mut out);
                        out.push(InlineNode::ColorPush { code: chars[i + 1] });
                        i += 2;
                    }
                } else {
                    text.push('§');
                    i += 1;
                }
            }
            '$' => {
                if let Some((key, next_i)) = take_dollar_ref(&chars, i) {
                    flush(&mut text, &mut out);
                    out.push(InlineNode::Ref { key });
                    i = next_i;
                } else {
                    text.push('$');
                    i += 1;
                }
            }
            '[' => {
                if let Some((expr, next_i)) = take_bracket_expr(&chars, i) {
                    flush(&mut text, &mut out);
                    out.push(InlineNode::Scope { expr });
                    i = next_i;
                } else {
                    text.push('[');
                    i += 1;
                }
            }
            '£' => {
                if let Some((token, next_i)) = take_pound_icon(&chars, i + 1) {
                    flush(&mut text, &mut out);
                    out.push(InlineNode::Icon { token });
                    i = next_i;
                } else {
                    text.push('£');
                    i += 1;
                }
            }
            other => {
                text.push(other);
                i += 1;
            }
        }
    }

    flush(&mut text, &mut out);
    out
}

fn flush(text: &mut String, out: &mut Vec<InlineNode>) {
    if !text.is_empty() {
        out.push(InlineNode::Text {
            value: std::mem::take(text),
        });
    }
}

fn is_ref_key_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-'
}

fn take_dollar_ref(chars: &[char], start: usize) -> Option<(String, usize)> {
    let mut j = start + 1;
    while j < chars.len() && is_ref_key_char(chars[j]) {
        j += 1;
    }
    if j == start + 1 {
        return None;
    }
    if j < chars.len() && chars[j] == '$' {
        let key: String = chars[start + 1..j].iter().collect();
        Some((key, j + 1))
    } else {
        None
    }
}

fn take_bracket_expr(chars: &[char], start: usize) -> Option<(String, usize)> {
    let mut j = start + 1;
    while j < chars.len() {
        match chars[j] {
            ']' => {
                let expr: String = chars[start + 1..j].iter().collect();
                return Some((expr, j + 1));
            }
            '\n' | '\r' => return None,
            _ => j += 1,
        }
    }
    None
}

fn take_pound_icon(chars: &[char], start: usize) -> Option<(String, usize)> {
    let mut j = start;
    while j < chars.len() {
        let c = chars[j];
        if c.is_ascii_alphanumeric() || c == '_' {
            j += 1;
        } else {
            break;
        }
    }
    if j == start {
        return None;
    }
    let token: String = chars[start..j].iter().collect();
    if j < chars.len() && chars[j] == '£' {
        Some((token, j + 1))
    } else {
        Some((token, j))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Vec<InlineNode> {
        parse_inline(s)
    }

    #[test]
    fn plain_text() {
        assert_eq!(
            t("hello 世界"),
            vec![InlineNode::Text {
                value: "hello 世界".into()
            }]
        );
    }

    #[test]
    fn color_push_pop() {
        let r = t("§Y步兵§!专家");
        assert_eq!(
            r,
            vec![
                InlineNode::ColorPush { code: 'Y' },
                InlineNode::Text {
                    value: "步兵".into()
                },
                InlineNode::ColorPop,
                InlineNode::Text {
                    value: "专家".into()
                },
            ]
        );
    }

    #[test]
    fn dollar_ref() {
        assert_eq!(
            t("$PRC_zhang_guotao$"),
            vec![InlineNode::Ref {
                key: "PRC_zhang_guotao".into()
            }]
        );
    }

    #[test]
    fn bracket_scope() {
        let r = t("将[FROM.GetName]整合");
        assert_eq!(
            r,
            vec![
                InlineNode::Text {
                    value: "将".into()
                },
                InlineNode::Scope {
                    expr: "FROM.GetName".into()
                },
                InlineNode::Text {
                    value: "整合".into()
                },
            ]
        );
    }

    #[test]
    fn icon_paired_and_self_terminated() {
        // 配对：£mil_factory£
        let r = t("£mil_factory£5");
        assert_eq!(
            r,
            vec![
                InlineNode::Icon {
                    token: "mil_factory".into()
                },
                InlineNode::Text { value: "5".into() }
            ]
        );
        // 自终止（遇 § 收尾）
        let r2 = t("£BoP_left_texticon §Y5§!");
        assert_eq!(
            r2,
            vec![
                InlineNode::Icon {
                    token: "BoP_left_texticon".into()
                },
                InlineNode::Text { value: " ".into() },
                InlineNode::ColorPush { code: 'Y' },
                InlineNode::Text { value: "5".into() },
                InlineNode::ColorPop,
            ]
        );
    }

    #[test]
    fn escapes() {
        let r = t("a\\nb\\tc\\\"d");
        assert_eq!(
            r,
            vec![
                InlineNode::Text { value: "a".into() },
                InlineNode::LineBreak,
                InlineNode::Text { value: "b".into() },
                InlineNode::Tab,
                InlineNode::Text {
                    value: "c\"d".into()
                },
            ]
        );
    }

    #[test]
    fn unclosed_dollar_falls_back_to_text() {
        // 未闭合 $key 不能识别为 ref，原样保留
        assert_eq!(
            t("price is $10 today"),
            vec![InlineNode::Text {
                value: "price is $10 today".into()
            }]
        );
    }

    #[test]
    fn unclosed_bracket_falls_back() {
        assert_eq!(
            t("oh [no oh"),
            vec![InlineNode::Text {
                value: "oh [no oh".into()
            }]
        );
    }

    #[test]
    fn nested_dollar_inside_bracket_kept_opaque() {
        // [..] 整体 opaque，不再二次解析里面的 $..$
        let r = t("[?JAP_desired_military_factories]");
        assert_eq!(
            r,
            vec![InlineNode::Scope {
                expr: "?JAP_desired_military_factories".into()
            }]
        );
    }

    #[test]
    fn mixed_real_world() {
        let r = t("解锁£BoP_left_texticon §Y$PRC_communist_power_struggle$§!决议");
        assert_eq!(
            r,
            vec![
                InlineNode::Text {
                    value: "解锁".into()
                },
                InlineNode::Icon {
                    token: "BoP_left_texticon".into()
                },
                InlineNode::Text { value: " ".into() },
                InlineNode::ColorPush { code: 'Y' },
                InlineNode::Ref {
                    key: "PRC_communist_power_struggle".into()
                },
                InlineNode::ColorPop,
                InlineNode::Text {
                    value: "决议".into()
                },
            ]
        );
    }
}
