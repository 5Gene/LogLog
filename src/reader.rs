use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use regex::Regex;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use tar::Archive;
use zip::ZipArchive;

/// 日志文件读取器
pub struct LogReader {
    temp_dir: Option<PathBuf>,
}

impl LogReader {
    pub fn new() -> Self {
        Self { temp_dir: None }
    }

    /// 读取日志文件，支持压缩包和文本文件
    /// file_path: 文件路径
    /// dir_pattern: 压缩包内目录正则（可选）
    /// file_pattern: 压缩包内文件正则（可选）
    pub fn read_logs<F>(
        &mut self,
        file_path: &Path,
        dir_pattern: Option<&str>,
        file_pattern: Option<&str>,
        mut callback: F,
    ) -> Result<()>
    where
        F: FnMut(&str, usize) -> Result<()>,
    {
        let file_ext = file_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        match file_ext {
            "zip" => self.read_zip(file_path, dir_pattern, file_pattern, callback),
            "gz" | "tgz" => self.read_tar_gz(file_path, dir_pattern, file_pattern, callback),
            "tar" => self.read_tar(file_path, dir_pattern, file_pattern, callback),
            _ => self.read_text_file(file_path, callback),
        }
    }

    /// 读取纯文本文件
    fn read_text_file<F>(&self, file_path: &Path, mut callback: F) -> Result<()>
    where
        F: FnMut(&str, usize) -> Result<()>,
    {
        let file = File::open(file_path)
            .context(format!("Failed to open file: {:?}", file_path))?;
        let reader = BufReader::with_capacity(64 * 1024, file); // 64KB buffer

        for (line_num, line) in reader.lines().enumerate() {
            let line = line?;
            callback(&line, line_num + 1)?;
        }

        Ok(())
    }

    /// 读取ZIP压缩包
    fn read_zip<F>(
        &mut self,
        file_path: &Path,
        dir_pattern: Option<&str>,
        file_pattern: Option<&str>,
        mut callback: F,
    ) -> Result<()>
    where
        F: FnMut(&str, usize) -> Result<()>,
    {
        let file = File::open(file_path)?;
        let mut archive = ZipArchive::new(file)?;

        let dir_regex = dir_pattern.map(|p| Regex::new(p)).transpose()?;
        let file_regex = file_pattern.map(|p| Regex::new(p)).transpose()?;

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let entry_path = entry.name().to_string();

            // 检查是否匹配目录和文件模式
            if !self.matches_pattern(&entry_path, &dir_regex, &file_regex) {
                continue;
            }

            if entry.is_file() {
                let reader = BufReader::with_capacity(64 * 1024, &mut entry);
                for (line_num, line) in reader.lines().enumerate() {
                    let line = line?;
                    callback(&line, line_num + 1)?;
                }
            }
        }

        Ok(())
    }

    /// 读取TAR.GZ压缩包
    fn read_tar_gz<F>(
        &mut self,
        file_path: &Path,
        dir_pattern: Option<&str>,
        file_pattern: Option<&str>,
        callback: F,
    ) -> Result<()>
    where
        F: FnMut(&str, usize) -> Result<()>,
    {
        let file = File::open(file_path)?;
        let decoder = GzDecoder::new(file);
        let archive = Archive::new(decoder);
        self.process_tar_archive(archive, dir_pattern, file_pattern, callback)
    }

    /// 读取TAR压缩包
    fn read_tar<F>(
        &mut self,
        file_path: &Path,
        dir_pattern: Option<&str>,
        file_pattern: Option<&str>,
        callback: F,
    ) -> Result<()>
    where
        F: FnMut(&str, usize) -> Result<()>,
    {
        let file = File::open(file_path)?;
        let archive = Archive::new(file);
        self.process_tar_archive(archive, dir_pattern, file_pattern, callback)
    }

    /// 处理TAR归档
    fn process_tar_archive<R, F>(
        &self,
        mut archive: Archive<R>,
        dir_pattern: Option<&str>,
        file_pattern: Option<&str>,
        mut callback: F,
    ) -> Result<()>
    where
        R: Read,
        F: FnMut(&str, usize) -> Result<()>,
    {
        let dir_regex = dir_pattern.map(|p| Regex::new(p)).transpose()?;
        let file_regex = file_pattern.map(|p| Regex::new(p)).transpose()?;

        for entry in archive.entries()? {
            let mut entry = entry?;
            let entry_path = entry.path()?.to_string_lossy().to_string();

            // 检查是否匹配目录和文件模式
            if !self.matches_pattern(&entry_path, &dir_regex, &file_regex) {
                continue;
            }

            if entry.header().entry_type().is_file() {
                let reader = BufReader::with_capacity(64 * 1024, &mut entry);
                for (line_num, line) in reader.lines().enumerate() {
                    let line = line?;
                    callback(&line, line_num + 1)?;
                }
            }
        }

        Ok(())
    }

    /// 检查路径是否匹配模式
    fn matches_pattern(
        &self,
        path: &str,
        dir_regex: &Option<Regex>,
        file_regex: &Option<Regex>,
    ) -> bool {
        // 如果没有提供模式，则匹配所有
        if dir_regex.is_none() && file_regex.is_none() {
            return true;
        }

        // 检查目录模式
        if let Some(dir_re) = dir_regex {
            if !dir_re.is_match(path) {
                return false;
            }
        }

        // 检查文件模式
        if let Some(file_re) = file_regex {
            let file_name = Path::new(path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if !file_re.is_match(file_name) {
                return false;
            }
        }

        true
    }
}

impl Drop for LogReader {
    fn drop(&mut self) {
        // 清理临时目录
        if let Some(temp_dir) = &self.temp_dir {
            let _ = std::fs::remove_dir_all(temp_dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_read_text_file() -> Result<()> {
        let temp_file = std::env::temp_dir().join("test_log.txt");
        let mut file = File::create(&temp_file)?;
        writeln!(file, "line 1")?;
        writeln!(file, "line 2")?;
        writeln!(file, "line 3")?;

        let mut reader = LogReader::new();
        let mut lines = Vec::new();
        reader.read_logs(&temp_file, None, None, |line, _| {
            lines.push(line.to_string());
            Ok(())
        })?;

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "line 1");
        
        std::fs::remove_file(temp_file)?;
        Ok(())
    }
}

