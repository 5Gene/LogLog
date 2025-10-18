#!/bin/bash

echo "===== LogLog 日志解析工具 ====="
echo ""

echo "正在编译..."
cargo build --release
if [ $? -ne 0 ]; then
    echo "编译失败！"
    exit 1
fi

echo ""
echo "编译成功！"
echo ""

echo "运行示例测试..."
./target/release/loglog -c config_example.yaml -l test_log.txt -o output.txt -v

if [ -f output.txt ]; then
    echo ""
    echo "===== 输出结果预览 ====="
    cat output.txt
fi

echo ""
echo "完成！"

