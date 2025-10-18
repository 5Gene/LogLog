use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// 日志配置根结构
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogConfig {
    pub categories: Vec<Category>,
    #[serde(skip)]
    pub raw_content: String, // 保存原始配置内容
}

/// 大类别
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Category {
    pub app: String,
    pub children: Vec<Child>,
}

/// 子类别
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Child {
    #[serde(default)]
    pub name: String,
    pub tag: String,
    pub msgs: Vec<MsgRule>,
}

/// 消息规则
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MsgRule {
    #[serde(default)]
    pub keys: Vec<String>, // 必须包含的关键字
    #[serde(default)]
    pub ignores: Vec<String>, // 必须不包含的关键字
    pub mean: String,        // 解释含义
    #[serde(default)]
    pub regex: Option<String>, // 正则表达式
    #[serde(default)]
    pub level: String, // 日志等级
    
    // 运行时编译的正则表达式（不序列化）
    #[serde(skip)]
    pub compiled_regex: Option<Regex>,
}

impl LogConfig {
    /// 从YAML文件加载配置
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(&path)
            .context(format!("Failed to read config file: {:?}", path.as_ref()))?;
        
        let mut config: LogConfig = serde_yaml::from_str(&content)
            .context("Failed to parse YAML config")?;
        
        // 保存原始内容
        config.raw_content = content;
        
        // 预编译所有正则表达式
        config.compile_regexes()?;
        
        Ok(config)
    }
    
    /// 预编译所有正则表达式以提高性能
    fn compile_regexes(&mut self) -> Result<()> {
        for category in &mut self.categories {
            for child in &mut category.children {
                for msg in &mut child.msgs {
                    if let Some(regex_str) = &msg.regex {
                        let compiled = Regex::new(regex_str)
                            .context(format!("Failed to compile regex: {}", regex_str))?;
                        msg.compiled_regex = Some(compiled);
                    }
                }
            }
        }
        Ok(())
    }
    
    /// 获取所有唯一的tag列表（用于快速过滤）
    pub fn get_all_tags(&self) -> Vec<String> {
        let mut tags = Vec::new();
        for category in &self.categories {
            for child in &category.children {
                tags.push(child.tag.clone());
            }
        }
        tags.sort();
        tags.dedup();
        tags
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let yaml = r#"
categories:
  - app: 蓝牙
    children:
      - name: bt_btm
        tag: bt_btm
        msgs:
          - keys: ["start_btm", "init_bt"]
            ignores: ["error"]
            mean: "蓝牙启动"
            level: "i"
          - keys: ["connect fail"]
            mean: "蓝牙连接失败"
            level: "e"
"#;
        let config: LogConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.categories.len(), 1);
        assert_eq!(config.categories[0].children.len(), 1);
    }
}

