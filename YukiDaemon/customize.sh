#!/system/bin/sh
#
# ########################################################################################
#   YukiDaemon 模块安装脚本
#   作者: Yuki
# ########################################################################################

# --- 模块路径和工具 ---
# $MODPATH 是 Magisk 传入的模块安装路径
CONFIG_DIR="$MODPATH/config"
# 模块已自带默认配置文件于此路径
FINAL_CONFIG_PATH="$CONFIG_DIR/config.yaml"
# 临时下载位置
TEMP_DOWNLOAD_FILE="/data/local/tmp/yuki_config_download.yaml"

# --- 远程配置 URL ---
GITHUB_CONFIG_BASE_URL="https://raw.githubusercontent.com/imacte/YukiCtrl/main/configs"
GITEE_CONFIG_BASE_URL="https://gitee.com/imacte_ui/YUKICTRL/raw/main/configs"

# --- 自动检测 BusyBox ---
if [ -x "/data/adb/magisk/busybox" ]; then
  BUSYBOX="/data/adb/magisk/busybox"
elif [ -x "/data/adb/ksu/bin/busybox" ]; then
  BUSYBOX="/data/adb/ksu/bin/busybox"
else [ -x "/data/adb/ap/bin/busybox" ];
  BUSYBOX="/data/adb/ap/bin/busybox"
fi

# 确保临时文件在脚本退出时被删除
trap "$BUSYBOX rm -f $TEMP_DOWNLOAD_FILE" EXIT

# --- 语言定义 (Language Definitions) ---
# 1. 优先尝试获取用户设置的语言
CURRENT_LOCALE=$(/system/bin/getprop persist.sys.locale)

# 2. 如果为空，回退到 ROM 默认语言
if [ -z "$CURRENT_LOCALE" ]; then
    CURRENT_LOCALE=$(/system/bin/getprop ro.product.locale)
fi

# 默认使用英文
LANG_CODE="en"
MSG_GET_DEVICE_ID="-> 1. Getting device identifiers..."
MSG_FOUND_PLATFORM="   - Found ro.board.platform:"
MSG_FOUND_SOC_MODEL="   - Found SoC Model:"
MSG_EXTRACTED_SHORT_SOC="   - Extracted Short SoC:"
MSG_EXTRACTED_FULL_SOC="   - Extracted Full SoC:"
MSG_FINAL_IDENTIFIERS="   - Final identifier list:"
MSG_RESOLVE_SUCCESS="-> 2. Successfully resolved device to target config:"
MSG_RESOLVE_FAIL="-> ❌ Could not resolve any known target config for this device."
MSG_PREPARE_DOWNLOAD="-> 3. Preparing to download:"
MSG_TRY_GITHUB="   - Trying to download from [GitHub]..."
MSG_GITHUB_SUCCESS="   - ✔ [GitHub] Config downloaded and validated successfully!"
MSG_GITHUB_FAIL="   - ❌ [GitHub] Download failed or content validation failed."
MSG_SEPARATOR="   - -------------------------------"
MSG_TRY_GITEE="   - Trying to download from [Gitee]..."
MSG_GITEE_SUCCESS="   - ✔ [Gitee] Config downloaded and validated successfully!"
MSG_GITEE_FAIL="   - ❌ [Gitee] Download failed or content validation failed."
MSG_DOWNLOAD_APPLY_SUCCESS="-> ✔ Successfully applied SoC-specific config:"
MSG_DOWNLOAD_APPLY_PATH="   - Specific config replaced at:"
MSG_DOWNLOAD_FAIL="-> ❌ Failed to download or find specific config."
MSG_DOWNLOAD_FALLBACK="-> Module will use the built-in default config.yaml."
MSG_CONFIG_READY="-> Configuration files are ready."

# 检查是否包含 zh
if echo "$CURRENT_LOCALE" | $BUSYBOX grep -qi "zh"; then
  LANG_CODE="zh"
  MSG_GET_DEVICE_ID="-> 1. 正在获取设备标识符..."
  MSG_FOUND_PLATFORM="   - 发现 ro.board.platform:"
  MSG_FOUND_SOC_MODEL="   - 发现 SoC Model:"
  MSG_EXTRACTED_SHORT_SOC="   - 提取到 Short SoC:"
  MSG_EXTRACTED_FULL_SOC="   - 提取到 Full SoC:"
  MSG_FINAL_IDENTIFIERS="   - 最终标识符列表:"
  MSG_RESOLVE_SUCCESS="-> 2. 成功将设备解析为目标配置:"
  MSG_RESOLVE_FAIL="-> ❌ 无法为该设备解析出任何已知的目标配置。"
  MSG_PREPARE_DOWNLOAD="-> 3. 准备下载:"
  MSG_TRY_GITHUB="   - 正在尝试从 [GitHub] 下载..."
  MSG_GITHUB_SUCCESS="   - ✔ [GitHub] 配置下载并校验成功！"
  MSG_GITHUB_FAIL="   - ❌ [GitHub] 下载失败或内容校验失败。"
  MSG_SEPARATOR="   - -------------------------------"
  MSG_TRY_GITEE="   - 正在尝试从 [Gitee] 下载..."
  MSG_GITEE_SUCCESS="   - ✔ [Gitee] 配置下载并校验成功！"
  MSG_GITEE_FAIL="   - ❌ [Gitee] 下载失败或内容校验失败。"
  MSG_DOWNLOAD_APPLY_SUCCESS="-> ✔ 成功应用SoC专用配置:"
  MSG_DOWNLOAD_APPLY_PATH="   - 专用配置已替换:"
  MSG_DOWNLOAD_FAIL="-> ❌ 专用配置下载失败或未找到。"
  MSG_DOWNLOAD_FALLBACK="-> 模块将使用已内置的默认 config.yaml。"
  MSG_CONFIG_READY="-> 配置文件准备完成。"
fi
# --- 语言定义结束 ---


get_device_identifiers() {
  ui_print " "
  ui_print "$MSG_GET_DEVICE_ID"
  
  IDENTIFIERS=""
  
  # 1a. 获取 ro.board.platform
  BOARD_PLATFORM=$(/system/bin/getprop ro.board.platform | $BUSYBOX tr '[:upper:]' '[:lower:]' | $BUSYBOX tr -d '[:space:]')
  
  # 无论是否为空，都打印出来
  ui_print " $MSG_FOUND_PLATFORM $BOARD_PLATFORM"
  
  if [ -n "$BOARD_PLATFORM" ]; then
      IDENTIFIERS="$IDENTIFIERS $BOARD_PLATFORM"
  fi

  # 1b. 获取 SoC Model
  RAW_SOC_MODEL=$(/system/bin/getprop ro.product.soc.model | $BUSYBOX tr '[:upper:]' '[:lower:]')
  if [ -z "$RAW_SOC_MODEL" ]; then
      RAW_SOC_MODEL=$(/system/bin/getprop ro.soc.model | $BUSYBOX tr '[:upper:]' '[:lower:]')
  fi
  # ---------------------------------

  # 无论是否为空，都打印出来
  ui_print " $MSG_FOUND_SOC_MODEL $RAW_SOC_MODEL"

  if [ -n "$RAW_SOC_MODEL" ]; then
    
    # 1c. 提取 shortSocPattern (e.g., sm8650)
    SHORT_SOC_MODEL=$($BUSYBOX echo "$RAW_SOC_MODEL" | $BUSYBOX sed -n 's/.*\(\(sm\|mt\|sdm\|sd\)[0-9]\{3,\}\).*/\1/p')
    if [ -n "$SHORT_SOC_MODEL" ]; then
          ui_print " $MSG_EXTRACTED_SHORT_SOC $SHORT_SOC_MODEL"
          IDENTIFIERS="$IDENTIFIERS $SHORT_SOC_MODEL"
    fi
    
    # 1d. 提取 fullSocModelFilename (e.g., snapdragon_8_gen_3)
    FULL_SOC_MODEL_FILENAME=$($BUSYBOX echo "$RAW_SOC_MODEL" | $BUSYBOX tr ' /' '__')
    if [ -n "$FULL_SOC_MODEL_FILENAME" ]; then
        ui_print " $MSG_EXTRACTED_FULL_SOC $FULL_SOC_MODEL_FILENAME"
        IDENTIFIERS="$IDENTIFIERS $FULL_SOC_MODEL_FILENAME"
    fi
  fi

  # 1e. 过滤重复项 (distinct)
  DISTINCT_IDENTIFIERS=$($BUSYBOX echo "$IDENTIFIERS" | $BUSYBOX awk '{for(i=1;i<=NF;i++)if(!a[$i]++)printf "%s ",$i}')
  ui_print " $MSG_FINAL_IDENTIFIERS $DISTINCT_IDENTIFIERS"
  
  echo "$DISTINCT_IDENTIFIERS"
}

resolve_canonical_name() {
    local identifier_list="$1"
    local canonical_name=""
    
    for identifier in $identifier_list; do
        case "$identifier" in
            "mt6833") canonical_name="mt6833"; break ;;
            "mt6891") canonical_name="mt6891"; break ;;
            "mt6895") canonical_name="mt6895"; break ;;
            "mt6983") canonical_name="mt6983"; break ;;
            "mt6983z") canonical_name="mt6983z"; break ;;
            "mt6985") canonical_name="mt6985"; break ;;
            "mt6985z"|"rubens") canonical_name="mt6985z"; break ;;
            "mt6989"|"mt8796") canonical_name="mt6989"; break ;;
            "mt6991") canonical_name="mt6991"; break ;;
            "sm7475") canonical_name="sm7475"; break ;;
            "sm8150") canonical_name="sm8150"; break ;;
            "sm8250-ac"|"kona") canonical_name="sm8250-ac"; break ;;
            "sm8350") canonical_name="sm8350"; break ;;
            "sm8450") canonical_name="sm8450"; break ;;
            "sm8475") canonical_name="sm8475"; break ;;
            "sm8550"|"kalama") canonical_name="sm8550"; break ;;
            "sm8650"|"pineapple") canonical_name="sm8650"; break ;;
            "sm8750") canonical_name="sm8750"; break ;;
            "sm8850 | canoe") canonical_name="sm8850"; break ;;
        esac
    done
    
    echo "$canonical_name"
}

validate_config_content() {
    local file_path="$1"
    local expected_name="$2"
    # 定义临时 awk 脚本的路径
    local awk_script_path="/data/local/tmp/validate.awk"
    
    if [ ! -s "$file_path" ]; then
        return 1 # 文件不存在或为空
    fi
    
    # 1. 创建临时 awk 脚本
    # ---------------------------------
    $BUSYBOX cat > "$awk_script_path" << EOF
BEGIN {
    IGNORECASE=1
    ltrim_regex = "^[ \t]+"
    rtrim_regex = "[ \t]+$"
}
/meta:/ {in_meta=1}
in_meta && /name:/ {
    gsub(/name:|["'\r]/, "");
    sub(ltrim_regex, "");
    sub(rtrim_regex, "");
    # 注意: 这里的 \$0 必须转义
    # 否则 sh 会在 cat 写入时将其替换掉
    if (\$0 == target) {
        print "true";
        exit;
    }
}
/^[a-zA-Z]+:/ {if (!/meta:/) in_meta=0}
EOF
    # ---------------------------------
    
    # 2. 使用 -f (文件) 标志来执行 awk 脚本
    VALID=$($BUSYBOX awk -v target="$expected_name" -f "$awk_script_path" "$file_path")
    
    # 3. 清理临时脚本
    $BUSYBOX rm -f "$awk_script_path"
    
    if [ "$VALID" = "true" ]; then
        return 0 # 校验成功
    else
        return 1 # 校验失败
    fi
}

download_config() {
    local target_name="$1"
    local remote_filename="$target_name.yaml"
    
    ui_print " "
    ui_print "$MSG_PREPARE_DOWNLOAD $remote_filename"
    
    # --- 尝试源 1: GitHub ---
    ui_print " $MSG_TRY_GITHUB"
    local config_url_github="$GITHUB_CONFIG_BASE_URL/$remote_filename"
    $BUSYBOX wget -T 10 -qO "$TEMP_DOWNLOAD_FILE" "$config_url_github"
    
    if validate_config_content "$TEMP_DOWNLOAD_FILE" "$target_name"; then
        ui_print " $MSG_GITHUB_SUCCESS"
        # 替换模块中的默认配置
        $BUSYBOX mv -f "$TEMP_DOWNLOAD_FILE" "$FINAL_CONFIG_PATH"
        return 0
    else
        ui_print " $MSG_GITHUB_FAIL"
        $BUSYBOX rm -f "$TEMP_DOWNLOAD_FILE"
    fi

    # --- 尝试源 2: Gitee ---
    ui_print " $MSG_SEPARATOR"
    ui_print " $MSG_TRY_GITEE"
    local config_url_gitee="$GITEE_CONFIG_BASE_URL/$remote_filename"
    $BUSYBOX wget -T 10 -qO "$TEMP_DOWNLOAD_FILE" "$config_url_gitee"
    
    if validate_config_content "$TEMP_DOWNLOAD_FILE" "$target_name"; then
        ui_print " $MSG_GITEE_SUCCESS"
        # 替换模块中的默认配置
        $BUSYBOX mv -f "$TEMP_DOWNLOAD_FILE" "$FINAL_CONFIG_PATH"
        return 0
    else
        ui_print " $MSG_GITEE_FAIL"
        $BUSYBOX rm -f "$TEMP_DOWNLOAD_FILE"
    fi
    
    return 1
}

# 步骤 1 & 2: 获取标识符并解析
ALL_IDENTIFIERS=$(get_device_identifiers)
TARGET_CONFIG_NAME=$(resolve_canonical_name "$ALL_IDENTIFIERS")

if [ -z "$TARGET_CONFIG_NAME" ]; then
    ui_print " "
    ui_print "$MSG_RESOLVE_FAIL"
    DOWNLOAD_SUCCESS=false
else
    ui_print " "
    ui_print "$MSG_RESOLVE_SUCCESS $TARGET_CONFIG_NAME"
    
    # 步骤 3 & 4: 下载并校验
    if download_config "$TARGET_CONFIG_NAME"; then
        DOWNLOAD_SUCCESS=true
    else
        DOWNLOAD_SUCCESS=false
    fi
fi

# 步骤 5: 处理下载结果 (成功或回退)
if [ "$DOWNLOAD_SUCCESS" = true ]; then
    ui_print " "
    ui_print "$MSG_DOWNLOAD_APPLY_SUCCESS $TARGET_CONFIG_NAME"
    ui_print " $MSG_DOWNLOAD_APPLY_PATH $FINAL_CONFIG_PATH"
else
    ui_print " "
    ui_print "$MSG_DOWNLOAD_FAIL"
    ui_print "$MSG_DOWNLOAD_FALLBACK"
    # (什么也不做，保留 $FINAL_CONFIG_PATH 的原文件)
fi

ui_print " "
ui_print "$MSG_CONFIG_READY"