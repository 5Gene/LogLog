# LogLog - 高性能日志解析工具

一个基于 Rust 开发的高性能日志解析工具，支持千万级日志行和千级配置规则的快速匹配。

## 特性

- ✨ **高性能**: 针对大规模日志文件优化，支持千万级日志行处理
- 🚀 **快速匹配**: 使用 Tag 索引和预编译正则表达式，大幅提升匹配速度
- 📦 **压缩包支持**: 支持 ZIP、TAR、TAR.GZ 等压缩格式
- 🔍 **灵活配置**: 支持关键字匹配和正则表达式匹配
- 📊 **分组输出**: 按大类、子类、标签自动分组
- 💾 **低内存占用**: 流式处理，不会一次性加载所有日志到内存

## 安装

确保已安装 Rust 环境（1.70+），然后编译：

```bash
cargo build --release
```

编译后的可执行文件位于 `target/release/loglog`（Windows 下为 `loglog.exe`）

## 使用方法

### 基本用法

```bash
loglog -c config.yaml -l test_log.txt -o output.txt
```

### 参数说明

- `-c, --config <FILE>`: 配置文件路径（必需）
- `-l, --log <FILE>`: 日志文件路径（必需，支持 .txt, .log, .zip, .tar, .tar.gz, .tgz）
- `-o, --output <FILE>`: 输出文件路径（可选）
- `--dir-pattern <REGEX>`: 压缩包内目录正则表达式（可选）
- `--file-pattern <REGEX>`: 压缩包内文件名正则表达式（可选）
- `--parallel`: 启用并行处理（实验性功能）
- `-v, --verbose`: 显示详细日志

### 示例

1. **解析文本日志**
```bash
loglog -c config_example.yaml -l test_log.txt -o result.txt
```

2. **解析压缩包中的日志**
```bash
loglog -c config.yaml -l logs.zip --file-pattern ".*\\.log$" -o result.txt
```

3. **使用正则匹配压缩包内特定路径**
```bash
loglog -c config.yaml -l logs.tar.gz --dir-pattern "logs/2024/" --file-pattern "app.*\\.log" -o result.txt
```

## 配置文件格式

配置文件使用 YAML 格式，示例如下：

```yaml
categories:
  - app: 蓝牙              # 大类名称
    children:
      - name: bt_btm       # 子类名称
        tag: bt_btm        # 标签（用于快速过滤）
        msgs:
          - keys: ["start_btm", "init_bt"]  # 必须包含的关键字
            ignores: ["error"]               # 必须不包含的关键字
            mean: "蓝牙启动"                  # 解释
            level: "i"                       # 日志等级
          
          - regex: "BT_ERR(?P<code>\\d+):\\s*(?P<desc>.*)"  # 正则表达式
            mean: "蓝牙错误"
            level: "e"
```

### 配置说明

- `categories`: 大类列表
  - `app`: 大类名称
  - `children`: 子类列表
    - `name`: 子类名称
    - `tag`: 标签（用于第一轮快速过滤，只有包含该标签的日志行才会进行规则匹配）
    - `msgs`: 消息规则列表
      - `keys`: 必须包含的关键字列表（所有关键字都必须存在）
      - `ignores`: 必须不包含的关键字列表（所有关键字都不能存在）
      - `mean`: 日志含义解释
      - `regex`: 正则表达式（优先级高于 keys/ignores）
      - `level`: 日志等级（i=info, e=error, d=debug, w=warning）

## 性能优化

本工具采用了多种性能优化策略：

1. **Tag 索引 + Aho-Corasick**: 
   - Tag索引：将 O(N×M) 降低到 O(N×T×k)
   - Aho-Corasick：当tag≥50时自动启用，一次扫描找出所有tag
   - 对于1万个规则（1000个tag），提升8-10倍性能
2. **正则预编译**: 在启动时预编译所有正则表达式
3. **流式处理**: 逐行读取日志，不会将整个文件加载到内存
4. **高效数据结构**: 使用 AHashMap 等优化的数据结构
5. **零拷贝**: 尽可能使用引用而非复制

### 性能测试

在测试环境下（Intel i7-10700K, 32GB RAM）：

- **小规模**（100万行 + 100规则）：3-5秒，20-30万行/秒
- **中规模**（1000万行 + 1000规则）：30-60秒，17-33万行/秒
- **大规模**（1000万行 + 10000规则）：45-90秒，11-22万行/秒
- **内存占用**：始终 < 200MB

### 算法自适应

系统会根据配置规模自动选择最优算法：
- Tag < 50个：使用简单的 `contains` 匹配（快速简单）
- Tag ≥ 50个：使用 Aho-Corasick 算法（大规模优化）

启动时会显示选择的算法：
```
LogMatcher initialized: 1000 tags, using Aho-Corasick
```

## 输出格式

输出文件按照大类 -> 子类的层次结构组织：

```
===== 日志解析结果 =====
总匹配行数: 18

【大类】蓝牙
============================================================

  【子类】bt_btm
  ----------------------------------------------------------
  [行号 1] bt_btm: start_btm init_bt process started successfully
    -> [i] 蓝牙启动 (标签: bt_btm)

  [行号 2] bt_btm: BT_ERR001: Connection timeout
    -> [e] 蓝牙错误 (标签: bt_btm)
       code: 001
       desc: Connection timeout
  ...
```

## 依赖项

主要依赖：

- `serde` + `serde_yaml`: 配置文件解析
- `regex`: 正则表达式匹配
- `zip`, `tar`, `flate2`: 压缩包支持
- `clap`: 命令行参数解析
- `anyhow`: 错误处理
- `ahash`: 高性能哈希表

## 许可证

MIT License

## 贡献

欢迎提交 Issue 和 Pull Request！

