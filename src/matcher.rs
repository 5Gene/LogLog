use crate::config::{Child, LogConfig, MsgRule};
use crate::pool::{StringPool, VecPool};
use crate::zero_copy::{MatchResultZeroCopy, MatchDetailZeroCopy};
use ahash::{AHashMap, AHashSet};
use aho_corasick::AhoCorasick;
use anyhow::Result;
use std::sync::Arc;

/// 匹配结果（支持序列化供UI使用）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MatchResult {
    pub line: String,           // 原始日志行
    pub line_number: usize,     // 行号
    pub matches: Vec<MatchDetail>, // 匹配到的规则详情
}

/// 匹配详情（支持序列化供UI使用）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MatchDetail {
    pub app: String,        // 大类
    pub child_name: String, // 子类名称
    pub tag: String,        // 标签
    pub mean: String,       // 解释含义
    pub level: String,      // 日志等级
    pub regex_captures: Option<AHashMap<String, String>>, // 正则捕获组
}

/// 高性能日志匹配器
pub struct LogMatcher {
    config: Arc<LogConfig>,
    /// Tag索引：tag -> (category_idx, child_idx)
    tag_index: AHashMap<String, Vec<(usize, usize)>>,
    /// Aho-Corasick自动机（用于大量tag的快速匹配）
    ac_automaton: Option<AhoCorasick>,
    /// Tag列表（与ac_automaton中的模式一一对应）
    tag_list: Vec<String>,
    /// 是否使用Aho-Corasick（tag数量 >= 阈值时启用）
    use_aho_corasick: bool,
    /// 字符串对象池（减少堆分配）
    string_pool: Arc<StringPool>,
    /// Vec对象池（减少Vec分配）
    vec_pool: Arc<VecPool<MatchDetail>>,
}

impl LogMatcher {
    /// Aho-Corasick启用阈值：tag数量超过此值时自动启用
    const AC_THRESHOLD: usize = 50;

    /// 创建新的匹配器
    pub fn new(config: LogConfig) -> Self {
        let config = Arc::new(config);
        let tag_index = Self::build_tag_index(&config);
        
        // 决定是否使用Aho-Corasick
        let tag_count = tag_index.len();
        let use_aho_corasick = tag_count >= Self::AC_THRESHOLD;
        
        let (ac_automaton, tag_list) = if use_aho_corasick {
            let tags: Vec<String> = tag_index.keys().cloned().collect();
            let ac = AhoCorasick::new(&tags).expect("Failed to build Aho-Corasick automaton");
            (Some(ac), tags)
        } else {
            (None, Vec::new())
        };
        
        log::info!(
            "LogMatcher initialized: {} tags, using {}",
            tag_count,
            if use_aho_corasick { "Aho-Corasick" } else { "contains" }
        );
        
        Self {
            config,
            tag_index,
            ac_automaton,
            tag_list,
            use_aho_corasick,
            string_pool: Arc::new(StringPool::new(1000)),  // 字符串池
            vec_pool: Arc::new(VecPool::new(500)),         // Vec池
        }
    }

    /// 获取统计信息（用于MatchStats）
    pub fn get_tag_count(&self) -> usize {
        self.tag_index.len()
    }

    /// 是否启用了Aho-Corasick
    pub fn is_ac_enabled(&self) -> bool {
        self.use_aho_corasick
    }

    /// 匹配单行日志（零拷贝版本，性能优化）
    /// 
    /// 使用生命周期和Cow避免不必要的字符串拷贝
    /// 适合需要极致性能的场景
    pub fn match_line_zero_copy<'a>(&'a self, line: &'a str, line_number: usize) -> Option<MatchResultZeroCopy<'a>> {
        let mut result = MatchResultZeroCopy::new(line, line_number);
        let mut has_match = false;

        if self.use_aho_corasick {
            if let Some(ac) = &self.ac_automaton {
                let mut matched_tags = AHashSet::new();
                
                for mat in ac.find_iter(line) {
                    let tag = &self.tag_list[mat.pattern()];
                    matched_tags.insert(tag);
                }
                
                for tag in matched_tags {
                    if let Some(positions) = self.tag_index.get(tag) {
                        for &(cat_idx, child_idx) in positions {
                            let category = &self.config.categories[cat_idx];
                            let child = &category.children[child_idx];

                            for msg_rule in &child.msgs {
                                if let Some(detail) = self.match_rule_zero_copy(
                                    line,
                                    &category.app,
                                    &child.name,
                                    &child.tag,
                                    msg_rule
                                ) {
                                    result.add_match(detail);
                                    has_match = true;
                                }
                            }
                        }
                    }
                }
            }
        } else {
            for (tag, positions) in &self.tag_index {
                if !line.contains(tag.as_str()) {
                    continue;
                }

                for &(cat_idx, child_idx) in positions {
                    let category = &self.config.categories[cat_idx];
                    let child = &category.children[child_idx];

                    for msg_rule in &child.msgs {
                        if let Some(detail) = self.match_rule_zero_copy(
                            line,
                            &category.app,
                            &child.name,
                            &child.tag,
                            msg_rule
                        ) {
                            result.add_match(detail);
                            has_match = true;
                        }
                    }
                }
            }
        }

        if has_match {
            Some(result)
        } else {
            None
        }
    }

    /// 零拷贝版本的规则匹配
    fn match_rule_zero_copy<'a>(
        &self,
        line: &'a str,
        app: &'a str,
        child_name: &'a str,
        tag: &'a str,
        rule: &'a MsgRule,
    ) -> Option<MatchDetailZeroCopy<'a>> {
        // 优先使用正则表达式
        if let Some(regex) = &rule.compiled_regex {
            if regex.is_match(line) {
                let mut detail = MatchDetailZeroCopy::new(
                    app,
                    child_name,
                    tag,
                    &rule.mean,
                    &rule.level,
                );

                // 提取命名捕获组
                if let Some(captures) = regex.captures(line) {
                    let mut regex_captures = AHashMap::new();
                    for name in regex.capture_names().flatten() {
                        if let Some(matched) = captures.name(name) {
                            regex_captures.insert(name.to_string(), matched.as_str().to_string());
                        }
                    }
                    if !regex_captures.is_empty() {
                        detail.regex_captures = Some(regex_captures);
                    }
                }

                return Some(detail);
            }
        } else {
            // 使用关键字匹配
            let all_keys_match = rule.keys.is_empty() 
                || rule.keys.iter().all(|key| line.contains(key.as_str()));

            let no_ignores_match = rule.ignores.is_empty()
                || !rule.ignores.iter().any(|ignore| line.contains(ignore.as_str()));

            if all_keys_match && no_ignores_match {
                return Some(MatchDetailZeroCopy::new(
                    app,
                    child_name,
                    tag,
                    &rule.mean,
                    &rule.level,
                ));
            }
        }

        None
    }

    /// 构建tag索引以加速查找
    fn build_tag_index(config: &LogConfig) -> AHashMap<String, Vec<(usize, usize)>> {
        let mut index = AHashMap::new();
        
        for (cat_idx, category) in config.categories.iter().enumerate() {
            for (child_idx, child) in category.children.iter().enumerate() {
                index
                    .entry(child.tag.clone())
                    .or_insert_with(Vec::new)
                    .push((cat_idx, child_idx));
            }
        }
        
        index
    }

    /// 匹配单行日志
    /// 
    /// 性能优化策略：
    /// 1. 首先通过tag快速过滤（根据tag数量选择算法）：
    ///    - tag < 50个: 使用 contains（简单快速）
    ///    - tag >= 50个: 使用 Aho-Corasick（多模式匹配，O(n)复杂度）
    /// 2. 对于正则表达式，使用预编译的Regex对象
    /// 3. 对于关键字匹配，使用简单的contains（对于少量关键字最快）
    pub fn match_line(&self, line: &str, line_number: usize) -> Option<MatchResult> {
        let mut matches = Vec::new();

        if self.use_aho_corasick {
            // 使用Aho-Corasick：一次扫描找出所有匹配的tag
            // 时间复杂度：O(n + z)，n是行长度，z是匹配数量
            if let Some(ac) = &self.ac_automaton {
                let mut matched_tags = AHashSet::new();
                
                // 找出所有匹配的tag（去重）
                for mat in ac.find_iter(line) {
                    let tag = &self.tag_list[mat.pattern()];
                    matched_tags.insert(tag);
                }
                
                // 对每个匹配的tag处理规则
                for tag in matched_tags {
                    if let Some(positions) = self.tag_index.get(tag) {
                        for &(cat_idx, child_idx) in positions {
                            let category = &self.config.categories[cat_idx];
                            let child = &category.children[child_idx];

                            for msg_rule in &child.msgs {
                                if let Some(match_detail) = self.match_rule(line, category.app.as_str(), child, msg_rule) {
                                    matches.push(match_detail);
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // 使用传统的contains：适合少量tag（< 50个）
            // 时间复杂度：O(T × n × m)，T是tag数量，n是行长度，m是tag长度
            for (tag, positions) in &self.tag_index {
                if !line.contains(tag.as_str()) {
                    continue;
                }

                for &(cat_idx, child_idx) in positions {
                    let category = &self.config.categories[cat_idx];
                    let child = &category.children[child_idx];

                    for msg_rule in &child.msgs {
                        if let Some(match_detail) = self.match_rule(line, category.app.as_str(), child, msg_rule) {
                            matches.push(match_detail);
                        }
                    }
                }
            }
        }

        if matches.is_empty() {
            None
        } else {
            Some(MatchResult {
                line: line.to_string(),
                line_number,
                matches,
            })
        }
    }

    /// 匹配单个规则
    fn match_rule(
        &self,
        line: &str,
        app: &str,
        child: &Child,
        rule: &MsgRule,
    ) -> Option<MatchDetail> {
        // 优先使用正则表达式
        if let Some(regex) = &rule.compiled_regex {
            if let Some(captures) = regex.captures(line) {
                // 提取命名捕获组
                let mut regex_captures = AHashMap::new();
                for name in regex.capture_names().flatten() {
                    if let Some(matched) = captures.name(name) {
                        regex_captures.insert(name.to_string(), matched.as_str().to_string());
                    }
                }

                return Some(MatchDetail {
                    app: app.to_string(),
                    child_name: child.name.clone(),
                    tag: child.tag.clone(),
                    mean: rule.mean.clone(),
                    level: rule.level.clone(),
                    regex_captures: Some(regex_captures),
                });
            }
        } else {
            // 使用关键字匹配
            // 检查所有keys是否都包含
            let all_keys_match = rule.keys.is_empty() 
                || rule.keys.iter().all(|key| line.contains(key.as_str()));

            // 检查所有ignores是否都不包含
            let no_ignores_match = rule.ignores.is_empty()
                || !rule.ignores.iter().any(|ignore| line.contains(ignore.as_str()));

            if all_keys_match && no_ignores_match {
                return Some(MatchDetail {
                    app: app.to_string(),
                    child_name: child.name.clone(),
                    tag: child.tag.clone(),
                    mean: rule.mean.clone(),
                    level: rule.level.clone(),
                    regex_captures: None,
                });
            }
        }

        None
    }

}

/// 性能统计（详细分段）
#[derive(Debug, Default)]
pub struct MatchStats {
    pub total_lines: usize,
    pub matched_lines: usize,
    pub total_matches: usize,
    pub processing_time_ms: u128,
    // 详细分段统计
    pub tag_matching_time_ms: u128,    // Tag匹配耗时
    pub rule_matching_time_ms: u128,   // 规则匹配耗时
    pub io_time_ms: u128,              // I/O耗时
    pub ac_enabled: bool,              // 是否启用了Aho-Corasick
    pub tag_count: usize,              // Tag数量
}

impl MatchStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_match(&mut self, result: &MatchResult) {
        self.matched_lines += 1;
        self.total_matches += result.matches.len();
    }

    pub fn print_summary(&self) {
        println!("\n===== 匹配统计 =====");
        println!("总行数: {}", self.total_lines);
        println!("匹配行数: {}", self.matched_lines);
        println!("总匹配数: {}", self.total_matches);
        println!("处理时间: {} ms", self.processing_time_ms);
        
        if self.total_lines > 0 {
            println!(
                "匹配率: {:.2}%",
                (self.matched_lines as f64 / self.total_lines as f64) * 100.0
            );
            println!(
                "速度: {:.2} 行/秒",
                self.total_lines as f64 / (self.processing_time_ms as f64 / 1000.0)
            );
        }
        
        // 详细分段统计
        if self.processing_time_ms > 0 {
            println!("\n===== 性能分析 =====");
            println!("Tag匹配算法: {}", if self.ac_enabled { "Aho-Corasick" } else { "contains" });
            println!("Tag数量: {}", self.tag_count);
            
            if self.tag_matching_time_ms > 0 {
                let tag_percent = (self.tag_matching_time_ms as f64 / self.processing_time_ms as f64) * 100.0;
                println!("Tag匹配耗时: {} ms ({:.1}%)", self.tag_matching_time_ms, tag_percent);
            }
            
            if self.rule_matching_time_ms > 0 {
                let rule_percent = (self.rule_matching_time_ms as f64 / self.processing_time_ms as f64) * 100.0;
                println!("规则匹配耗时: {} ms ({:.1}%)", self.rule_matching_time_ms, rule_percent);
            }
            
            if self.io_time_ms > 0 {
                let io_percent = (self.io_time_ms as f64 / self.processing_time_ms as f64) * 100.0;
                println!("I/O耗时: {} ms ({:.1}%)", self.io_time_ms, io_percent);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;

    #[test]
    fn test_matcher() {
        let yaml = r#"
categories:
  - app: 蓝牙
    children:
      - name: bt_btm
        tag: bt_btm
        msgs:
          - keys: ["start_btm", "init_bt"]
            mean: "蓝牙启动"
            level: "i"
          - keys: ["connect", "fail"]
            ignores: ["success"]
            mean: "蓝牙连接失败"
            level: "e"
"#;
        let config: LogConfig = serde_yaml::from_str(yaml).unwrap();
        let mut config = config;
        config.compile_regexes().unwrap();
        
        let matcher = LogMatcher::new(config);
        
        // 测试匹配
        let line = "bt_btm: start_btm init_bt process";
        let result = matcher.match_line(line, 1);
        assert!(result.is_some());
        
        let result = result.unwrap();
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].mean, "蓝牙启动");
    }
}

