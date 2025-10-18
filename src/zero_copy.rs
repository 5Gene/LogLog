use std::borrow::Cow;
use ahash::AHashMap;

/// 零拷贝匹配结果（使用生命周期避免字符串拷贝）
/// 
/// 性能优势：
/// - 使用 Cow<'a, str> 可以避免不必要的字符串拷贝
/// - 当不需要修改时，直接引用原始字符串
/// - 只在需要修改时才进行拷贝（Copy-on-Write）
#[derive(Debug, Clone)]
pub struct MatchResultZeroCopy<'a> {
    pub line: Cow<'a, str>,              // 零拷贝原始日志行
    pub line_number: usize,              // 行号
    pub matches: Vec<MatchDetailZeroCopy<'a>>, // 匹配详情
}

impl<'a> MatchResultZeroCopy<'a> {
    /// 创建新的匹配结果（零拷贝）
    pub fn new(line: &'a str, line_number: usize) -> Self {
        Self {
            line: Cow::Borrowed(line),
            line_number,
            matches: Vec::new(),
        }
    }

    /// 添加匹配详情
    pub fn add_match(&mut self, detail: MatchDetailZeroCopy<'a>) {
        self.matches.push(detail);
    }

    /// 转换为拥有所有权的版本（用于长期存储）
    pub fn into_owned(self) -> MatchResultOwned {
        MatchResultOwned {
            line: self.line.into_owned(),
            line_number: self.line_number,
            matches: self.matches.into_iter().map(|m| m.into_owned()).collect(),
        }
    }
}

/// 零拷贝匹配详情
#[derive(Debug, Clone)]
pub struct MatchDetailZeroCopy<'a> {
    pub app: Cow<'a, str>,
    pub child_name: Cow<'a, str>,
    pub tag: Cow<'a, str>,
    pub mean: Cow<'a, str>,
    pub level: Cow<'a, str>,
    pub regex_captures: Option<AHashMap<String, String>>, // 正则捕获必须拥有
}

impl<'a> MatchDetailZeroCopy<'a> {
    /// 创建新的匹配详情（零拷贝）
    pub fn new(
        app: &'a str,
        child_name: &'a str,
        tag: &'a str,
        mean: &'a str,
        level: &'a str,
    ) -> Self {
        Self {
            app: Cow::Borrowed(app),
            child_name: Cow::Borrowed(child_name),
            tag: Cow::Borrowed(tag),
            mean: Cow::Borrowed(mean),
            level: Cow::Borrowed(level),
            regex_captures: None,
        }
    }

    /// 转换为拥有所有权的版本
    pub fn into_owned(self) -> MatchDetailOwned {
        MatchDetailOwned {
            app: self.app.into_owned(),
            child_name: self.child_name.into_owned(),
            tag: self.tag.into_owned(),
            mean: self.mean.into_owned(),
            level: self.level.into_owned(),
            regex_captures: self.regex_captures,
        }
    }
}

/// 拥有所有权的匹配结果（用于长期存储和序列化）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MatchResultOwned {
    pub line: String,
    pub line_number: usize,
    pub matches: Vec<MatchDetailOwned>,
}

/// 拥有所有权的匹配详情
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MatchDetailOwned {
    pub app: String,
    pub child_name: String,
    pub tag: String,
    pub mean: String,
    pub level: String,
    pub regex_captures: Option<AHashMap<String, String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_copy() {
        let line = "test log line";
        let mut result = MatchResultZeroCopy::new(line, 1);
        
        let detail = MatchDetailZeroCopy::new("app", "child", "tag", "mean", "i");
        result.add_match(detail);

        // 验证是借用而非拷贝
        match &result.line {
            Cow::Borrowed(_) => {}, // 期望是Borrowed
            Cow::Owned(_) => panic!("Should be borrowed"),
        }

        // 转换为owned版本
        let owned = result.into_owned();
        assert_eq!(owned.line, "test log line");
    }
}

