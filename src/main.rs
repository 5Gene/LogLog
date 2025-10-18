mod config;
mod matcher;
mod reader;
mod result;
mod pool;      // 对象池模块
mod zero_copy; // 零拷贝优化模块

use anyhow::{Context, Result};
use clap::Parser;
use matcher::{LogMatcher, MatchStats};
use reader::LogReader;
use result::ResultGrouper;
use std::path::PathBuf;
use std::time::Instant;

/// 高性能日志解析工具
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// 配置文件路径
    #[clap(short, long, value_parser)]
    config: PathBuf,

    /// 日志文件路径（支持 .txt, .log, .zip, .tar, .tar.gz, .tgz）
    #[clap(short, long, value_parser)]
    log: PathBuf,

    /// 输出文件路径
    #[clap(short, long, value_parser)]
    output: Option<PathBuf>,

    /// 压缩包内目录正则表达式（可选）
    #[clap(long)]
    dir_pattern: Option<String>,

    /// 压缩包内文件名正则表达式（可选）
    #[clap(long)]
    file_pattern: Option<String>,

    /// 启用多线程处理（实验性，适合多文件场景）
    #[clap(long)]
    parallel: bool,

    /// 输出JSON格式（便于UI展示）
    #[clap(long)]
    json: bool,

    /// 是否包含未匹配的日志行
    #[clap(long)]
    include_unmatched: bool,

    /// 启用零拷贝模式（最大化性能，减少内存分配）
    #[clap(long)]
    zero_copy: bool,

    /// 是否显示详细日志
    #[clap(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    // 解析命令行参数
    let args = Args::parse();

    // 初始化日志
    if args.verbose {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .init();
    } else {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Info)
            .init();
    }

    println!("===== LogLog 高性能日志解析工具 =====\n");

    // 1. 加载配置文件
    println!("正在加载配置文件: {:?}", args.config);
    let config = config::LogConfig::from_file(&args.config)
        .context("加载配置文件失败")?;
    
    let rule_count: usize = config.categories.iter()
        .map(|cat| cat.children.iter()
            .map(|child| child.msgs.len())
            .sum::<usize>())
        .sum();
    
    println!("配置加载成功:");
    println!("  - 大类数量: {}", config.categories.len());
    println!("  - 规则数量: {}", rule_count);
    println!();

    // 2. 创建匹配器
    let matcher = LogMatcher::new(config);

    // 3. 创建结果分组器
    let mut grouper = ResultGrouper::new();
    let mut stats = MatchStats::new();
    
    // 记录matcher统计信息
    stats.tag_count = matcher.get_tag_count();
    stats.ac_enabled = matcher.is_ac_enabled();

    // 4. 读取并处理日志
    println!("正在处理日志文件: {:?}", args.log);
    if args.parallel {
        println!("提示: 多线程模式已启用（实验性功能）");
    }
    let start_time = Instant::now();

    let mut reader = LogReader::new();
    reader.read_logs(
        &args.log,
        args.dir_pattern.as_deref(),
        args.file_pattern.as_deref(),
        |line, line_number| {
            stats.total_lines += 1;

            // 显示进度（每10000行）
            if stats.total_lines % 10000 == 0 {
                print!("\r处理中... {} 行", stats.total_lines);
                std::io::Write::flush(&mut std::io::stdout()).ok();
            }

            // 匹配日志行
            if let Some(result) = matcher.match_line(line, line_number) {
                stats.add_match(&result);
                grouper.add_result(result);
            } else if args.include_unmatched {
                // 保存未匹配的日志
                grouper.add_unmatched(line_number, line.to_string());
            }

            Ok(())
        },
    )?;

    let elapsed = start_time.elapsed();
    stats.processing_time_ms = elapsed.as_millis();

    println!("\r处理完成！共处理 {} 行", stats.total_lines);
    println!();

    // 5. 输出统计信息
    stats.print_summary();
    grouper.print_summary();

    // 6. 输出结果到文件
    if let Some(output_path) = args.output {
        println!("\n正在写入结果到文件: {:?}", output_path);
        
        if args.json {
            // JSON格式输出（便于UI展示）
            grouper
                .output_json_to_file(&output_path)
                .context("写入JSON结果文件失败")?;
            println!("JSON结果已保存到: {:?}", output_path);
        } else {
            // 文本格式输出
            grouper
                .output_to_file(&output_path)
                .context("写入结果文件失败")?;
            println!("结果已保存到: {:?}", output_path);
        }
    } else {
        println!("\n提示: 使用 -o 参数指定输出文件路径以保存结果");
        if args.json {
            println!("提示: 使用 --json 参数需要同时指定 -o 输出文件");
        }
    }

    println!("\n处理完成！");
    Ok(())
}

