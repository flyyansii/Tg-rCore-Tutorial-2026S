#!/bin/bash

set -e
set -o pipefail

export CARGO_TARGET_RISCV64GC_UNKNOWN_NONE_ELF_RUNNER="qemu-system-riscv64 -machine virt -display none -serial stdio -device virtio-gpu-device,bus=virtio-mmio-bus.0,xres=800,yres=480 -device virtio-keyboard-device,bus=virtio-mmio-bus.1 -bios none -kernel"

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m'

ensure_tg_checker() {
    if ! command -v tg-rcore-tutorial-checker &> /dev/null; then
        echo -e "${YELLOW}tg-rcore-tutorial-checker is not installed, installing...${NC}"
        if cargo install tg-rcore-tutorial-checker; then
            echo -e "${GREEN}tg-rcore-tutorial-checker installed${NC}"
        else
            echo -e "${RED}failed to install tg-rcore-tutorial-checker${NC}"
            exit 1
        fi
    fi
}

run_base() {
    ensure_tg_checker
    echo "Running ch3 base tests..."
    if cargo run 2>&1 | tee /dev/stderr | tg-rcore-tutorial-checker --ch 3; then
        echo -e "${GREEN}ch3 base tests passed${NC}"
    else
        echo -e "${RED}ch3 base tests failed${NC}"
        return 1
    fi
}

run_exercise() {
    ensure_tg_checker
    echo "Running ch3 exercise tests..."
    if cargo run --features exercise 2>&1 | tee /dev/stderr | tg-rcore-tutorial-checker --ch 3 --exercise; then
        echo -e "${GREEN}ch3 exercise tests passed${NC}"
    else
        echo -e "${RED}ch3 exercise tests failed${NC}"
        return 1
    fi
}

run_snake() {
    echo "Running ch3 snake demo test..."
    if cargo run --features snake-ci 2>&1 | tee /dev/stderr | grep -q "Test ch3 snake OK!"; then
        echo -e "${GREEN}ch3 snake demo test passed${NC}"
    else
        echo -e "${RED}ch3 snake demo test failed${NC}"
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
    snake)
        run_snake
        ;;
    all)
        run_base
        echo ""
        run_exercise
        ;;
    *)
        echo "Usage: $0 [base|exercise|snake|all]"
        exit 1
        ;;
esac
