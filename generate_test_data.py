#!/usr/bin/env python3
"""
生成大规模测试配置和日志文件
用于测试Aho-Corasick算法的性能
"""

import yaml
import random

def generate_large_config(tag_count=1000, rules_per_tag=10):
    """生成大规模配置文件"""
    categories = []
    
    # 生成不同的应用类别
    apps = ["蓝牙", "网络", "存储", "显示", "音频", "传感器", "电源", "系统", "安全", "通信"]
    
    for i in range(tag_count):
        app = apps[i % len(apps)]
        tag = f"Module_{i:04d}"
        
        # 为每个tag生成多个规则
        msgs = []
        for j in range(rules_per_tag):
            rule = {
                "keys": [f"KEY_{i}_{j}", "operation"],
                "mean": f"模块{i}操作{j}",
                "level": random.choice(["i", "d", "e", "w"])
            }
            
            # 30%的规则使用正则
            if random.random() < 0.3:
                rule["regex"] = f"ERR{i:04d}_(?P<code>\\d+): (?P<msg>.*)"
            
            msgs.append(rule)
        
        # 查找或创建category
        category = next((cat for cat in categories if cat["app"] == app), None)
        if not category:
            category = {"app": app, "children": []}
            categories.append(category)
        
        category["children"].append({
            "name": f"子模块_{i}",
            "tag": tag,
            "msgs": msgs
        })
    
    config = {"categories": categories}
    
    # 保存到文件
    with open("config_large.yaml", "w", encoding="utf-8") as f:
        yaml.dump(config, f, allow_unicode=True, default_flow_style=False)
    
    print(f"✅ 生成配置文件: config_large.yaml")
    print(f"   - 标签数量: {tag_count}")
    print(f"   - 总规则数: {tag_count * rules_per_tag}")
    print(f"   - 大类数量: {len(categories)}")


def generate_large_log(line_count=100000, tag_count=1000):
    """生成大规模测试日志"""
    with open("test_log_large.txt", "w", encoding="utf-8") as f:
        for i in range(line_count):
            # 随机选择一些tag出现在日志中
            if random.random() < 0.3:  # 30%的行包含tag
                tag_idx = random.randint(0, tag_count - 1)
                tag = f"Module_{tag_idx:04d}"
                
                # 随机生成日志内容
                if random.random() < 0.3:
                    # 带错误码的日志
                    line = f"[2024-10-18 10:{i%60:02d}:{i%60:02d}] {tag}: ERR{tag_idx:04d}_{random.randint(1,99):03d}: operation failed"
                else:
                    # 普通日志
                    rule_idx = random.randint(0, 9)
                    line = f"[2024-10-18 10:{i%60:02d}:{i%60:02d}] {tag}: KEY_{tag_idx}_{rule_idx} operation completed"
            else:
                # 不包含tag的普通日志
                line = f"[2024-10-18 10:{i%60:02d}:{i%60:02d}] Other log message {i}"
            
            f.write(line + "\n")
    
    print(f"✅ 生成测试日志: test_log_large.txt")
    print(f"   - 日志行数: {line_count}")
    print(f"   - 预估匹配率: 30%")


if __name__ == "__main__":
    print("=" * 60)
    print("LogLog 大规模测试数据生成器")
    print("=" * 60)
    print()
    
    # 生成1000个tag，每个tag 10条规则 = 10000条规则
    generate_large_config(tag_count=1000, rules_per_tag=10)
    print()
    
    # 生成10万行日志用于测试
    generate_large_log(line_count=100000, tag_count=1000)
    print()
    
    print("=" * 60)
    print("测试命令：")
    print("  cargo build --release")
    print("  ./target/release/loglog -c config_large.yaml -l test_log_large.txt -o output_large.txt -v")
    print()
    print("观察输出中的算法选择：")
    print("  LogMatcher initialized: 1000 tags, using Aho-Corasick")
    print("=" * 60)

