use crate::matcher::{MatchDetail, MatchResult};
use crate::zero_copy::{MatchResultZeroCopy, MatchResultOwned};
use ahash::AHashMap;
use anyhow::Result;
use serde_json::json;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// 结果分组器
pub struct ResultGrouper {
    /// 按大类分组：app -> 匹配结果列表
    by_app: AHashMap<String, Vec<MatchResult>>,
    /// 按子类分组：(app, child_name) -> 匹配结果列表
    by_child: AHashMap<(String, String), Vec<MatchResult>>,
    /// 按标签分组：tag -> 匹配结果列表
    by_tag: AHashMap<String, Vec<MatchResult>>,
    /// 所有匹配结果
    all_results: Vec<MatchResult>,
    /// 未匹配的日志行（可选保存）
    unmatched_lines: Vec<(usize, String)>,
    /// 零拷贝模式（性能优化）
    zero_copy_mode: bool,
}

/// 零拷贝结果分组器（用于流式处理，不保存结果）
pub struct ResultGrouperZeroCopy {
    /// 统计信息
    pub matched_count: usize,
    pub unmatched_count: usize,
    /// 流式输出writer（可选）
    writer: Option<BufWriter<File>>,
}

impl ResultGrouper {
    pub fn new() -> Self {
        Self {
            by_app: AHashMap::new(),
            by_child: AHashMap::new(),
            by_tag: AHashMap::new(),
            all_results: Vec::new(),
            unmatched_lines: Vec::new(),
            zero_copy_mode: false,
        }
    }

    /// 创建零拷贝模式的分组器（不保存结果，只统计）
    pub fn new_zero_copy() -> Self {
        Self {
            by_app: AHashMap::new(),
            by_child: AHashMap::new(),
            by_tag: AHashMap::new(),
            all_results: Vec::new(),
            unmatched_lines: Vec::new(),
            zero_copy_mode: true,
        }
    }

    /// 添加零拷贝匹配结果（转换为owned后存储）
    pub fn add_result_zero_copy(&mut self, result: MatchResultZeroCopy) {
        // 转换为owned版本后添加
        let owned = result.into_owned();
        self.add_result_owned(owned);
    }

    /// 添加owned版本的结果
    fn add_result_owned(&mut self, result: MatchResultOwned) {
        use ahash::AHashSet;
        let mut apps = AHashSet::new();
        let mut children = AHashSet::new();
        let mut tags = AHashSet::new();
        
        // 转换为标准MatchResult
        let match_result = MatchResult {
            line: result.line.clone(),
            line_number: result.line_number,
            matches: result.matches.iter().map(|m| MatchDetail {
                app: m.app.clone(),
                child_name: m.child_name.clone(),
                tag: m.tag.clone(),
                mean: m.mean.clone(),
                level: m.level.clone(),
                regex_captures: m.regex_captures.clone(),
            }).collect(),
        };
        
        for match_detail in &match_result.matches {
            apps.insert(match_detail.app.clone());
            children.insert((match_detail.app.clone(), match_detail.child_name.clone()));
            tags.insert(match_detail.tag.clone());
        }
        
        for app in apps {
            self.by_app
                .entry(app)
                .or_insert_with(Vec::new)
                .push(match_result.clone());
        }
        
        for child_key in children {
            self.by_child
                .entry(child_key)
                .or_insert_with(Vec::new)
                .push(match_result.clone());
        }
        
        for tag in tags {
            self.by_tag
                .entry(tag)
                .or_insert_with(Vec::new)
                .push(match_result.clone());
        }

        self.all_results.push(match_result);
    }

    /// 添加未匹配的日志行
    pub fn add_unmatched(&mut self, line_number: usize, line: String) {
        self.unmatched_lines.push((line_number, line));
    }

    /// 获取未匹配的日志行数量
    pub fn unmatched_count(&self) -> usize {
        self.unmatched_lines.len()
    }

    /// 添加匹配结果（优化：避免重复克隆）
    pub fn add_result(&mut self, result: MatchResult) {
        // 收集唯一的索引键，避免同一行有多个匹配时重复添加
        use ahash::AHashSet;
        let mut apps = AHashSet::new();
        let mut children = AHashSet::new();
        let mut tags = AHashSet::new();
        
        for match_detail in &result.matches {
            apps.insert(match_detail.app.clone());
            children.insert((match_detail.app.clone(), match_detail.child_name.clone()));
            tags.insert(match_detail.tag.clone());
        }
        
        // 按大类索引（每个app只添加一次）
        for app in apps {
            self.by_app
                .entry(app)
                .or_insert_with(Vec::new)
                .push(result.clone());
        }
        
        // 按子类索引（每个child只添加一次）
        for child_key in children {
            self.by_child
                .entry(child_key)
                .or_insert_with(Vec::new)
                .push(result.clone());
        }
        
        // 按标签索引（每个tag只添加一次）
        for tag in tags {
            self.by_tag
                .entry(tag)
                .or_insert_with(Vec::new)
                .push(result.clone());
        }

        // 保存到全部结果
        self.all_results.push(result);
    }

    /// 获取所有结果
    pub fn get_all(&self) -> &[MatchResult] {
        &self.all_results
    }

    /// 按大类获取结果
    pub fn get_by_app(&self, app: &str) -> Option<&[MatchResult]> {
        self.by_app.get(app).map(|v| v.as_slice())
    }

    /// 按子类获取结果
    pub fn get_by_child(&self, app: &str, child_name: &str) -> Option<&[MatchResult]> {
        self.by_child
            .get(&(app.to_string(), child_name.to_string()))
            .map(|v| v.as_slice())
    }

    /// 按标签获取结果
    pub fn get_by_tag(&self, tag: &str) -> Option<&[MatchResult]> {
        self.by_tag.get(tag).map(|v| v.as_slice())
    }

    /// 获取所有大类名称
    pub fn get_all_apps(&self) -> Vec<String> {
        let mut apps: Vec<_> = self.by_app.keys().cloned().collect();
        apps.sort();
        apps
    }

    /// 获取指定大类下的所有子类
    pub fn get_children_of_app(&self, app: &str) -> Vec<String> {
        let mut children = Vec::new();
        for (app_name, child_name) in self.by_child.keys() {
            if app_name == app {
                children.push(child_name.clone());
            }
        }
        children.sort();
        children.dedup();
        children
    }

    /// 输出结果到文件
    pub fn output_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        writeln!(writer, "===== 日志解析结果 =====")?;
        writeln!(writer, "总匹配行数: {}\n", self.all_results.len())?;

        // 按大类输出
        for app in self.get_all_apps() {
            writeln!(writer, "\n【大类】{}", app)?;
            writeln!(writer, "{}", "=".repeat(60))?;

            let children = self.get_children_of_app(&app);
            for child_name in children {
                if let Some(results) = self.get_by_child(&app, &child_name) {
                    writeln!(writer, "\n  【子类】{}", child_name)?;
                    writeln!(writer, "  {}", "-".repeat(58))?;

                    for result in results {
                        writeln!(writer, "  [行号 {}] {}", result.line_number, result.line)?;
                        for match_detail in &result.matches {
                            if match_detail.app == app && match_detail.child_name == child_name {
                                writeln!(
                                    writer,
                                    "    -> [{}] {} (标签: {})",
                                    match_detail.level, match_detail.mean, match_detail.tag
                                )?;
                                if let Some(captures) = &match_detail.regex_captures {
                                    for (key, value) in captures {
                                        writeln!(writer, "       {}: {}", key, value)?;
                                    }
                                }
                            }
                        }
                        writeln!(writer)?;
                    }
                }
            }
        }

        writer.flush()?;
        Ok(())
    }

    /// 打印摘要信息
    pub fn print_summary(&self) {
        println!("\n===== 结果摘要 =====");
        println!("总匹配行数: {}", self.all_results.len());
        println!("大类数量: {}", self.by_app.len());
        println!("子类数量: {}", self.by_child.len());
        println!("标签数量: {}", self.by_tag.len());

        println!("\n各大类匹配统计:");
        for app in self.get_all_apps() {
            if let Some(results) = self.by_app.get(&app) {
                println!("  {}: {} 行", app, results.len());
                
                let children = self.get_children_of_app(&app);
                for child_name in children {
                    if let Some(child_results) = self.get_by_child(&app, &child_name) {
                        println!("    └─ {}: {} 行", child_name, child_results.len());
                    }
                }
            }
        }
        
        if !self.unmatched_lines.is_empty() {
            println!("\n未匹配行数: {}", self.unmatched_lines.len());
        }
    }

    /// 导出为JSON格式（UI接口）
    pub fn to_json(&self) -> Result<String> {
        let mut categories = Vec::new();
        
        for app in self.get_all_apps() {
            let mut children_data = Vec::new();
            let children = self.get_children_of_app(&app);
            
            for child_name in children {
                if let Some(results) = self.get_by_child(&app, &child_name) {
                    let matches: Vec<_> = results.iter().map(|r| {
                        json!({
                            "line_number": r.line_number,
                            "line": r.line,
                            "details": r.matches
                        })
                    }).collect();
                    
                    children_data.push(json!({
                        "name": child_name,
                        "match_count": results.len(),
                        "matches": matches
                    }));
                }
            }
            
            categories.push(json!({
                "app": app,
                "children": children_data
            }));
        }
        
        let output = json!({
            "total_matches": self.all_results.len(),
            "unmatched_count": self.unmatched_lines.len(),
            "categories": categories
        });
        
        Ok(serde_json::to_string_pretty(&output)?)
    }

    /// 输出JSON到文件（UI接口）
    pub fn output_json_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let json_str = self.to_json()?;
        std::fs::write(path, json_str)?;
        Ok(())
    }

}

impl Default for ResultGrouper {
    fn default() -> Self {
        Self::new()
    }
}

impl ResultGrouperZeroCopy {
    /// 创建流式输出的零拷贝分组器
    pub fn new_with_output<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        
        Ok(Self {
            matched_count: 0,
            unmatched_count: 0,
            writer: Some(writer),
        })
    }

    /// 处理零拷贝结果（流式输出）
    pub fn process_result(&mut self, result: &MatchResultZeroCopy) -> Result<()> {
        self.matched_count += 1;
        
        if let Some(writer) = &mut self.writer {
            // 流式输出到文件
            writeln!(writer, "[行号 {}] {}", result.line_number, result.line)?;
            for m in &result.matches {
                writeln!(
                    writer,
                    "  -> [{}] {} (app: {}, child: {})",
                    m.level, m.mean, m.app, m.child_name
                )?;
            }
            writeln!(writer)?;
        }
        
        Ok(())
    }

    /// 完成处理并输出统计
    pub fn finish(mut self) -> Result<()> {
        if let Some(mut writer) = self.writer.take() {
            writer.flush()?;
        }
        Ok(())
    }
}

