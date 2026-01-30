#!/usr/bin/env bash
#
# check-version.sh - 检查项目版本号一致性
#
# 从 Cargo.toml 的 [package].version 提取版本号，
# 并验证文档中的版本引用是否一致。
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# CI 环境下禁用颜色输出
if [[ -n "${CI:-}" ]] || [[ ! -t 1 ]]; then
    RED=''
    GREEN=''
    YELLOW=''
    NC=''
else
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    NC='\033[0m'
fi

# 从 Cargo.toml 提取版本号（单一真实源）
# 使用 sed 提取 [package] 段下的 version 字段
VERSION=$(sed -n '/^\[package\]/,/^\[/{ s/^version = "\([^"]*\)"/\1/p }' "$PROJECT_ROOT/Cargo.toml" | head -1)

if [[ -z "$VERSION" ]]; then
    echo -e "${RED}错误: 无法从 Cargo.toml 提取版本号${NC}"
    exit 1
fi

echo -e "当前版本 (Cargo.toml): ${GREEN}$VERSION${NC}"
echo ""

ERRORS=0

# 检查函数：在文件中查找版本号
check_file() {
    local file="$1"
    local pattern="$2"
    local description="$3"

    if [[ ! -f "$PROJECT_ROOT/$file" ]]; then
        echo -e "${YELLOW}跳过: $file (文件不存在)${NC}"
        return
    fi

    # 统计匹配次数
    local count
    count=$(grep -c "$pattern" "$PROJECT_ROOT/$file" 2>/dev/null || echo "0")

    if [[ "$count" -eq 0 ]]; then
        echo -e "${RED}不一致: $file - 未找到版本 $VERSION ($description)${NC}"
        ((ERRORS++))
    else
        echo -e "${GREEN}一致: $file ($description)${NC}"
    fi
}

# 检查 README.md 中的 badge 和安装命令
check_file "README.md" "$VERSION" "badge/安装命令"

# 检查 README.zh-CN.md
check_file "README.zh-CN.md" "$VERSION" "badge/安装命令"

# 检查 CLAUDE.md 项目描述
check_file "CLAUDE.md" "$VERSION" "项目描述"

# 检查 CONTRIBUTING.md
check_file "CONTRIBUTING.md" "$VERSION" "issue 模板示例"

# 检查 CONTRIBUTING.zh-CN.md
check_file "CONTRIBUTING.zh-CN.md" "$VERSION" "issue 模板示例"

echo ""

if [[ "$ERRORS" -gt 0 ]]; then
    echo -e "${RED}发现 $ERRORS 处版本不一致${NC}"
    echo ""
    echo "请更新以上文件中的版本号为: $VERSION"
    exit 1
else
    echo -e "${GREEN}所有版本号一致${NC}"
    exit 0
fi
