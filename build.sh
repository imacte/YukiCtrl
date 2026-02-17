#!/bin/bash
set -e # 如果任何命令失败，立即退出脚本

# --- 配置 ---
PROJ_DIR="yuki-daemon"
MODULE_DIR="YukiDaemon"
TARGET_ARCH="aarch64-linux-android"
BINARY_NAME="yuki-daemon"

# 最终输出的 zip 文件名 (带时间戳)
ZIP_FILE_NAME="YukiDaemon-$(date +%Y%m%d-%H%M).zip"

# --- 路径定义 ---
BINARY_SRC="$PROJ_DIR/target/$TARGET_ARCH/release/$BINARY_NAME"
TARGET_BIN_DIR="$MODULE_DIR/core/bin"
BINARY_DEST="$TARGET_BIN_DIR/$BINARY_NAME"

echo "========================================================"
echo "      开始构建 YukiDaemon                                "
echo "========================================================"

# --- 1. 编译 Rust 项目 ---
echo "--- 1. 编译 Rust 项目: $PROJ_DIR ---"
if [ ! -d "$PROJ_DIR" ]; then
    echo "[ERROR] 找不到项目目录: $PROJ_DIR"
    exit 1
fi

(cd "$PROJ_DIR" && cargo build --target $TARGET_ARCH --release)

# --- 2. 准备目标目录 ---
echo "--- 2. 准备目标目录 ---"
mkdir -p "$TARGET_BIN_DIR"

# --- 3. 复制二进制文件 ---
echo "--- 3. 复制二进制文件 ---"
if [ ! -f "$BINARY_SRC" ]; then
    echo "[ERROR] 找不到编译好的文件: $BINARY_SRC"
    exit 1
fi
cp "$BINARY_SRC" "$BINARY_DEST"

# --- 4. Strip 二进制文件 (减小体积) ---
# 注意：Linux/WSL 下通常需要指定 NDK 里的 llvm-strip 路径，
# 或者确保 aarch64-linux-android-strip 在 PATH 中。
STRIP_TOOL="/home/loyetu/work/tools/android-ndk-r29/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-strip"
if command -v $STRIP_TOOL &> /dev/null; then
    echo "--- 4. Stripping binary ---"
    $STRIP_TOOL "$BINARY_DEST"
else
    echo "[警告] 未找到 $STRIP_TOOL，跳过 strip 步骤。"
fi

# --- 5. 设置权限 ---
echo "--- 5. 设置权限 (755) ---"
chmod 755 "$BINARY_DEST"

# --- 6. 打包 Magisk 模块 ---
echo "--- 6. 打包 Magisk 模块 (Zip) ---"
rm -f "$ZIP_FILE_NAME"
# 进入模块目录打包，确保 zip 根目录是模块内容
(cd "$MODULE_DIR" && zip -r9 "../$ZIP_FILE_NAME" .)

echo "========================================================"
echo " 构建完成! 输出文件: $ZIP_FILE_NAME"
echo "========================================================"