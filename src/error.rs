use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hoi4ParserError {
    Parse { message: String },
    Generate { message: String },
}

impl Display for Hoi4ParserError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse { message } => write!(f, "解析失败: {message}"),
            Self::Generate { message } => write!(f, "还原失败: {message}"),
        }
    }
}

impl Error for Hoi4ParserError {}
