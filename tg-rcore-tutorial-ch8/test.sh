#!/bin/bash
# ch8 测试脚本
#
# 用法：
#   ./test.sh          # 运行全部测试（等价于 ./test.sh all）
#   ./test.sh base     # 运行基础测试
#   ./test.sh exercise # 运行练习测试
#   ./test.sh all      # 运行全部测试（base + exercise）
#
# 可选环境变量：
#   TIMEOUT_SEC  设为正整数（秒）时：仅对 cargo run 限时，输出写入临时文件并 tee 到终端，
#                        结束或超时后由 tg-rcore-tutorial-checker 检查该文件。未设置则行为与原先一致（管道直连 checker）。

set -e

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m'

ensure_tg_checker() {
    if ! command -v tg-rcore-tutorial-checker &> /dev/null; then
        echo -e "${YELLOW}tg-rcore-tutorial-checker 未安装，正在安装...${NC}"
        if cargo install tg-rcore-tutorial-checker; then
            echo -e "${GREEN}✓ tg-rcore-tutorial-checker 安装成功${NC}"
        else
            echo -e "${RED}✗ tg-rcore-tutorial-checker 安装失败${NC}"
            exit 1
        fi
    fi
}

ensure_tg_checker

set -o pipefail

# 超时路径：调用前设置 CARGO_EXTRA（数组，可为空）；参数传给 checker（如 --exercise）。
_tg_timed_cargo_check() {
    local log re ce
    log=$(mktemp) || return 1
    trap 'rm -f "$log"' EXIT
    set +e
    echo -e "${YELLOW}cargo run 超时: ${TIMEOUT_SEC}s（TIMEOUT_SEC）${NC}"
    timeout -k 10 --foreground "${TIMEOUT_SEC}" cargo run "${CARGO_EXTRA[@]}" 2>&1 | tee "$log" | tee /dev/stderr
    re=${PIPESTATUS[0]}
    set -e
    [[ $re -eq 124 ]] && echo -e "${RED}✗ cargo run 已超时，仅统计超时前输出。${NC}" >&2
    set +e
    tg-rcore-tutorial-checker --ch 8 "$@" <"$log"
    ce=$?
    set -e
    trap - EXIT
    rm -f "$log"
    [[ $re -eq 0 && $ce -eq 0 ]]
}

_run_base() {
    local passed
    if [[ -n "${TIMEOUT_SEC:-}" ]] && [[ "${TIMEOUT_SEC}" =~ ^[0-9]+$ ]] && [[ "${TIMEOUT_SEC}" -gt 0 ]]; then
        CARGO_EXTRA=()
        if _tg_timed_cargo_check; then passed=1; else passed=0; fi
    else
        if cargo run 2>&1 | tee /dev/stderr | tg-rcore-tutorial-checker --ch 8; then passed=1; else passed=0; fi
    fi
    [[ $passed -eq 1 ]]
}

_run_exercise() {
    local passed
    if [[ -n "${TIMEOUT_SEC:-}" ]] && [[ "${TIMEOUT_SEC}" =~ ^[0-9]+$ ]] && [[ "${TIMEOUT_SEC}" -gt 0 ]]; then
        CARGO_EXTRA=(--features exercise)
        if _tg_timed_cargo_check --exercise; then passed=1; else passed=0; fi
    else
        if cargo run --features exercise 2>&1 | tee /dev/stderr | tg-rcore-tutorial-checker --ch 8 --exercise; then passed=1; else passed=0; fi
    fi
    [[ $passed -eq 1 ]]
}

run_base() {
    echo "运行 ch8 基础测试..."
    cargo clean
    export CHAPTER=-8
    echo -e "${YELLOW}────────── cargo run 输出 ──────────${NC}"
    if _run_base; then
        echo ""
        echo -e "${YELLOW}────────── 测试结果 ──────────${NC}"
        echo -e "${GREEN}✓ ch8 基础测试通过${NC}"
        cargo clean
        return 0
    else
        echo ""
        echo -e "${YELLOW}────────── 测试结果 ──────────${NC}"
        echo -e "${RED}✗ ch8 基础测试失败${NC}"
        cargo clean
        return 1
    fi
}

run_exercise() {
    echo "运行 ch8 练习测试..."
    cargo clean
    export CHAPTER=8
    echo -e "${YELLOW}────────── cargo run --features exercise 输出 ──────────${NC}"
    if _run_exercise; then
        echo ""
        echo -e "${YELLOW}────────── 测试结果 ──────────${NC}"
        echo -e "${GREEN}✓ ch8 练习测试通过${NC}"
        cargo clean
        return 0
    else
        echo ""
        echo -e "${YELLOW}────────── 测试结果 ──────────${NC}"
        echo -e "${RED}✗ ch8 练习测试失败${NC}"
        cargo clean
        return 1
    fi
}

case "${1:-all}" in
    base)
        run_base
        ;;
    exercise)
        run_exercise
        ;;
    all)
        run_base
        echo ""
        run_exercise
        ;;
    *)
        echo "用法: $0 [base|exercise|all]"
        exit 1
        ;;
esac
