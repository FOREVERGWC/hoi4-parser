//! 本地化 yml 行级扫描。
//!
//! 文件结构：
//!   <BOM?>
//!   l_<lang>:                     ← header
//!   [空行 / 注释 / pair]*
//!
//! pair 行：  <indent>key[:version] "value" [#comment]
//! 不严格的"YAML"，是行式自定义格式。

use super::inline::{parse_inline, InlineNode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocFile {
    pub lang: String,
    pub entries: Vec<LocEntry>,
    pub warnings: Vec<LocWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocEntry {
    pub key: String,
    pub version: Option<u32>,
    pub raw: String,
    pub nodes: Vec<InlineNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocWarning {
    pub line: usize,
    pub kind: String,
    pub raw_line: String,
}

const BOM: &str = "\u{FEFF}";

pub fn parse_loc(input: &str) -> Result<LocFile, String> {
    let mut lang: Option<String> = None;
    let mut entries: Vec<LocEntry> = Vec::new();
    let mut warnings: Vec<LocWarning> = Vec::new();

    for (idx, raw_line) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = if idx == 0 {
            raw_line.strip_prefix(BOM).unwrap_or(raw_line)
        } else {
            raw_line
        };

        // 去掉行尾 \r（lines() 已处理 \n，但有些工具会留 \r）
        let line = line.trim_end_matches('\r');

        // 整行去左侧空白后判类型
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('#') {
            continue;
        }

        // header: l_xxx:
        if lang.is_none() {
            if let Some(l) = parse_header(trimmed) {
                lang = Some(l);
                continue;
            }
            return Err(format!("第 {} 行：缺少 l_<lang>: 文件头", line_no));
        }

        // 已读到 header 之后的行：尝试解析 pair
        match parse_pair_line(trimmed) {
            Some((key, version, raw_value)) => {
                let nodes = parse_inline(&raw_value);
                entries.push(LocEntry {
                    key,
                    version,
                    raw: raw_value,
                    nodes,
                });
            }
            None => {
                warnings.push(LocWarning {
                    line: line_no,
                    kind: "unrecognized_line".into(),
                    raw_line: raw_line.to_string(),
                });
            }
        }
    }

    let lang = lang.ok_or_else(|| "文件为空或缺少 l_<lang>: 文件头".to_string())?;
    Ok(LocFile {
        lang,
        entries,
        warnings,
    })
}

fn parse_header(trimmed: &str) -> Option<String> {
    if !trimmed.starts_with("l_") {
        return None;
    }
    let body = &trimmed[..];
    let colon = body.find(':')?;
    let head = &body[..colon];
    // header 行允许尾部还有内容（罕见），但 lang 必须是 l_ 开头的纯字符
    if !head.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some(head.to_string())
}

/// 解析单条 pair: `key[:version] "value" [#comment]`
fn parse_pair_line(s: &str) -> Option<(String, Option<u32>, String)> {
    // key
    let key_start = 0;
    let mut key_end = 0;
    for (i, c) in s.char_indices() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' {
            key_end = i + c.len_utf8();
        } else {
            break;
        }
    }
    if key_end == key_start {
        return None;
    }
    let key = s[..key_end].to_string();

    let rest = &s[key_end..];
    let mut version: Option<u32> = None;
    let after_key = if let Some(stripped) = rest.strip_prefix(':') {
        // 可选版本号
        let mut ver_end = 0;
        for (i, c) in stripped.char_indices() {
            if c.is_ascii_digit() {
                ver_end = i + c.len_utf8();
            } else {
                break;
            }
        }
        if ver_end > 0 {
            version = stripped[..ver_end].parse::<u32>().ok();
        }
        &stripped[ver_end..]
    } else {
        // 没有冒号，不是 pair（注意：header 已经在前面被吃掉了）
        return None;
    };

    // 跳过 key:version 与 " 之间的空白
    let after_ws = after_key.trim_start();
    if !after_ws.starts_with('"') {
        return None;
    }
    let value_str = &after_ws[1..];

    // 找配对的 "（注意 \" 转义）。直到行尾若仍未闭合则贪心闭合：用整段。
    let mut end = None;
    let bytes = value_str.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if i + 1 < bytes.len() => i += 2,
            b'"' => {
                end = Some(i);
                break;
            }
            _ => i += 1,
        }
    }
    let raw_value = match end {
        Some(e) => value_str[..e].to_string(),
        None => value_str.to_string(),
    };

    Some((key, version, raw_value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_file() {
        let src = "\u{FEFF}l_simp_chinese:\n JAP_keisuke_okada: \"冈田启介\"\n";
        let f = parse_loc(src).unwrap();
        assert_eq!(f.lang, "l_simp_chinese");
        assert_eq!(f.entries.len(), 1);
        assert_eq!(f.entries[0].key, "JAP_keisuke_okada");
        assert_eq!(f.entries[0].raw, "冈田启介");
        assert!(f.warnings.is_empty());
    }

    #[test]
    fn version_number_parsed() {
        let src = "l_english:\n FOO:3 \"bar\"\n";
        let f = parse_loc(src).unwrap();
        assert_eq!(f.entries[0].version, Some(3));
    }

    #[test]
    fn missing_header_errors() {
        let src = "FOO: \"bar\"\n";
        assert!(parse_loc(src).is_err());
    }

    #[test]
    fn comments_and_blanks_skipped() {
        let src = "l_english:\n\n  # hello\n FOO: \"bar\"\n";
        let f = parse_loc(src).unwrap();
        assert_eq!(f.entries.len(), 1);
    }

    #[test]
    fn unrecognized_line_becomes_warning() {
        let src = "l_english:\nthis is junk no colon no quote\nFOO: \"bar\"\n";
        let f = parse_loc(src).unwrap();
        assert_eq!(f.entries.len(), 1);
        assert_eq!(f.warnings.len(), 1);
        assert_eq!(f.warnings[0].kind, "unrecognized_line");
    }

    #[test]
    fn unclosed_quote_is_greedy_to_eol() {
        let src = "l_english:\n FOO: \"unterminated\nBAR: \"ok\"\n";
        let f = parse_loc(src).unwrap();
        // FOO 的 raw 是行尾止贪
        assert_eq!(f.entries[0].key, "FOO");
        assert_eq!(f.entries[0].raw, "unterminated");
        assert_eq!(f.entries[1].key, "BAR");
    }

    #[test]
    fn inline_nodes_attached() {
        let src = "l_simp_chinese:\n FOO: \"§Y步兵§!\"\n";
        let f = parse_loc(src).unwrap();
        assert_eq!(f.entries[0].nodes.len(), 3);
    }
}
