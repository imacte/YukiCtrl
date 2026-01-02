#!/bin/bash
set -e # 如果任何命令失败，立即退出脚本

# --- 配置 ---
MONITOR_PROJ_DIR="yuki-monitor"
SCHEDULER_PROJ_DIR="yuki-scheduler"
MODULE_DIR="YukiDaemon"
TARGET_ARCH="aarch64-linux-android"

# 最终输出的 zip 文件名
# 添加日期时间戳，方便版本管理
ZIP_FILE_NAME="YukiDaemon-Module-$(date +%Y%m%d-%H%M).zip"

# --- 路径定义 ---
# 编译后的二进制文件源路径
MONITOR_BINARY_SRC="$MONITOR_PROJ_DIR/target/$TARGET_ARCH/release/yuki-monitor"
SCHEDULER_BINARY_SRC="$SCHEDULER_PROJ_DIR/target/$TARGET_ARCH/release/yuki-scheduler"

# 模块内的目标目录
TARGET_BIN_DIR="$MODULE_DIR/core/bin"
# 模块内的目标文件路径
MONITOR_BINARY_DEST="$TARGET_BIN_DIR/yuki-monitor"
SCHEDULER_BINARY_DEST="$TARGET_BIN_DIR/yuki-scheduler"

# --- 1. 编译 yuki-monitor ---
echo "Building $MONITOR_PROJ_DIR..."
# (cd ...) 会在子 shell 中运行，不会改变当前目录
(cd "$MONITOR_PROJ_DIR" && cargo build --target $TARGET_ARCH --release)

# --- 2. 编译 yuki-scheduler ---
echo "Building $SCHEDULER_PROJ_DIR..."
(cd "$SCHEDULER_PROJ_DIR" && cargo build --target $TARGET_ARCH --release)

# --- 3. 准备目标目录 ---
echo "Creating destination directory: $TARGET_BIN_DIR"
mkdir -p "$TARGET_BIN_DIR"

# --- 4. 复制二进制文件 ---
echo "Copying binaries..."
cp "$MONITOR_BINARY_SRC" "$MONITOR_BINARY_DEST"
cp "$SCHEDULER_BINARY_SRC" "$SCHEDULER_BINARY_DEST"

# --- 5. (推荐) Strip 二进制文件 ---
STRIP_TOOL="aarch64-linux-android-strip"
if command -v $STRIP_TOOL &> /dev/null; then
    echo "Stripping binaries..."
    $STRIP_TOOL "$MONITOR_BINARY_DEST"
    $STRIP_TOOL "$SCHEDULER_BINARY_DEST"
else
    echo "Warning: '$STRIP_TOOL' not found in PATH. Binaries will not be stripped."
    echo "Tip: Add your Android NDK toolchains bin to your PATH for smaller binaries."
fi

# --- 6. 设置权限 ---
# 匹配你在 service.sh 中需要的 755 权限
echo "Setting executable permissions (755)..."
chmod 755 "$MONITOR_BINARY_DEST"
chmod 755 "$SCHEDULER_BINARY_DEST"

# --- 7. 打包模块 ---
echo "Packaging Magisk module..."
# 删除旧的 zip (如果存在)
rm -f "$ZIP_FILE_NAME"

# 进入模块目录，将 *内部所有* 文件打包
# 'cd' 和 'zip' 放在子 shell 中，防止目录切换
(cd "$MODULE_DIR" && zip -r9 "../$ZIP_FILE_NAME" .)

echo "----------------------------------------"
echo "Success! Module created: $ZIP_FILE_NAME"
echo "----------------------------------------"