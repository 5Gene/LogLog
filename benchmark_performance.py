#!/usr/bin/env python3
"""
性能对比测试：标准模式 vs 零拷贝模式
"""

import subprocess
import time
import os

def run_benchmark(mode, config, log_file, output):
    """运行性能测试"""
    cmd = [
        "./target/release/loglog",
        "-c", config,
        "-l", log_file,
        "-o", output
    ]
    
    if mode == "zero-copy":
        cmd.append("--zero-copy")
    
    print(f"\n{'='*60}")
    print(f"测试模式: {mode}")
    print(f"命令: {' '.join(cmd)}")
    print('='*60)
    
    start = time.time()
    result = subprocess.run(cmd, capture_output=True, text=True)
    elapsed = time.time() - start
    
    if result.returncode == 0:
        print(f"✅ 成功完成")
        print(f"⏱️  耗时: {elapsed:.2f} 秒")
        
        # 获取输出文件大小
        if os.path.exists(output):
            size_mb = os.path.getsize(output) / (1024 * 1024)
            print(f"📄 输出文件: {size_mb:.2f} MB")
        
        # 解析输出中的统计信息
        if "处理时间:" in result.stdout:
            print("\n统计信息:")
            for line in result.stdout.split('\n'):
                if any(k in line for k in ["处理时间", "匹配率", "速度", "吞吐"]):
                    print(f"  {line}")
    else:
        print(f"❌ 失败: {result.stderr}")
    
    return elapsed

def main():
    print("=" * 60)
    print("LogLog 性能对比测试")
    print("对象池 + 零拷贝优化")
    print("=" * 60)
    
    # 确保已编译
    print("\n🔨 编译 release 版本...")
    subprocess.run(["cargo", "build", "--release"], check=True)
    
    # 测试配置
    config = "config_large.yaml"
    log_file = "test_log_large.txt"
    
    # 生成测试数据（如果不存在）
    if not os.path.exists(config) or not os.path.exists(log_file):
        print("\n📦 生成测试数据...")
        subprocess.run(["python", "generate_test_data.py"], check=True)
    
    # 测试1: 标准模式
    time_standard = run_benchmark(
        "standard",
        config,
        log_file,
        "output_standard.txt"
    )
    
    # 测试2: 零拷贝模式
    time_zero_copy = run_benchmark(
        "zero-copy",
        config,
        log_file,
        "output_zero_copy.txt"
    )
    
    # 性能对比
    print("\n" + "=" * 60)
    print("📊 性能对比结果")
    print("=" * 60)
    print(f"\n标准模式:     {time_standard:.2f} 秒")
    print(f"零拷贝模式:   {time_zero_copy:.2f} 秒")
    
    improvement = ((time_standard - time_zero_copy) / time_standard) * 100
    print(f"\n✨ 性能提升:   {improvement:.1f}%")
    
    if improvement > 20:
        print("🎉 优化效果显著！")
    elif improvement > 10:
        print("👍 优化效果良好")
    else:
        print("💡 优化效果一般（可能是测试数据规模较小）")

if __name__ == "__main__":
    main()

