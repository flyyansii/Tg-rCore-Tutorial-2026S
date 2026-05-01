#!/bin/bash

set -euo pipefail

cargo build --features ci

KERNEL="target/riscv64gc-unknown-none-elf/debug/tg-rcore-tutorial-ch1"
OUTPUT=$(
    qemu-system-riscv64 \
        -machine virt \
        -nographic \
        -bios none \
        -kernel "$KERNEL" \
        2>&1
)

if echo "$OUTPUT" | grep -q "Hello, world!"; then
    echo "Test PASSED: Found 'Hello, world!' in output"
    exit 0
else
    echo "Test FAILED: 'Hello, world!' not found in output"
    echo "Actual output:"
    echo "$OUTPUT"
    exit 1
fi
