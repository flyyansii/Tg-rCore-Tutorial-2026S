#!/bin/bash

set -euo pipefail

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m'

ensure_tg_checker() {
    if ! command -v tg-rcore-tutorial-checker &> /dev/null; then
        echo -e "${YELLOW}tg-rcore-tutorial-checker not installed, installing...${NC}"
        cargo install tg-rcore-tutorial-checker
    fi
}

ensure_tg_checker

echo "Running ch2 base tests in CI-friendly nographic mode..."
echo -e "${YELLOW}────────── cargo build --features ci ──────────${NC}"
cargo build --features ci

KERNEL="target/riscv64gc-unknown-none-elf/debug/tg-rcore-tutorial-ch2"

echo -e "${YELLOW}────────── qemu output ──────────${NC}"
set -o pipefail
if qemu-system-riscv64 \
    -machine virt \
    -nographic \
    -bios none \
    -kernel "$KERNEL" \
    2>&1 | tee /dev/stderr | tg-rcore-tutorial-checker --ch 2; then
    echo ""
    echo -e "${YELLOW}────────── Test result ──────────${NC}"
    echo -e "${GREEN}✓ ch2 base tests passed${NC}"
    exit 0
else
    echo ""
    echo -e "${YELLOW}────────── Test result ──────────${NC}"
    echo -e "${RED}✗ ch2 base tests failed${NC}"
    exit 1
fi
