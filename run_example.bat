@echo off
echo ===== LogLog 日志解析工具 =====
echo.

echo 正在编译...
cargo build --release
if errorlevel 1 (
    echo 编译失败！
    pause
    exit /b 1
)

echo.
echo 编译成功！
echo.

echo 运行示例测试...
target\release\loglog.exe -c config_example.yaml -l test_log.txt -o output.txt -v

if exist output.txt (
    echo.
    echo ===== 输出结果预览 =====
    type output.txt
)

echo.
echo 完成！
pause

