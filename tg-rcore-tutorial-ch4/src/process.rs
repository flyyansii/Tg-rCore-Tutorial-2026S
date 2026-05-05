//! 进程管理模块
//!
//! 定义了 `Process` 结构体，封装了进程的地址空间、上下文和堆管理。
//!
//! ## 与第三章的区别
//!
//! 第三章的 `TaskControlBlock` 直接管理用户上下文和固定大小的用户栈。
//! 本章的 `Process` 引入了独立的地址空间（`AddressSpace`），每个进程拥有
//! 自己的 Sv39 页表，实现进程间内存隔离。
//!
//! 关键变化：
//! - 上下文变为 `ForeignContext`（包含 satp，支持跨地址空间切换）
//! - 用户栈映射到独立地址空间（不再在内核栈上分配）
//! - 支持堆管理（`sbrk` 系统调用）
//!
//! 教程阅读建议：
//!
//! - 先看 `new`：理解 ELF 装载、用户栈映射与 satp 构造；
//! - 再看 `change_program_brk`：理解 sbrk 对页映射范围的影响；
//! - 最后结合 `ch4/src/main.rs`：对齐“进程对象创建”和“调度执行”两条路径。

use crate::{build_flags, parse_flags, Sv39, Sv39Manager};
use alloc::alloc::alloc_zeroed;
use core::alloc::Layout;
use tg_kernel_context::{foreign::ForeignContext, LocalContext};
use tg_kernel_vm::{
    page_table::{MmuMeta, VAddr, PPN, VPN},
    AddressSpace,
};
const PT_LOAD: u32 = 1;
const PF_X: u32 = 1;
const PF_W: u32 = 2;
const PF_R: u32 = 4;

fn read_u16(input: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(input.get(offset..offset + 2)?.try_into().ok()?))
}

fn read_u32(input: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(input.get(offset..offset + 4)?.try_into().ok()?))
}

fn read_u64(input: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(input.get(offset..offset + 8)?.try_into().ok()?))
}

fn elf64_entry(input: &[u8]) -> Option<usize> {
    if input.len() < 32 || input.get(0..4) != Some(b"\x7fELF") {
        return None;
    }
    if input[4] != 2 || input[5] != 1 {
        return None;
    }
    Some(u64::from_le_bytes(input[24..32].try_into().ok()?) as usize)
}

/// 进程结构体
///
/// 包含进程运行所需的全部信息：
/// - `context`：`ForeignContext`，包含用户态寄存器和 satp（地址空间标识）
/// - `address_space`：Sv39 地址空间，管理该进程的页表
/// - `heap_bottom`：堆底地址（ELF 加载的最高地址的下一页）
/// - `program_brk`：当前堆顶地址（通过 sbrk 调整）
pub struct Process {
    /// 用户态上下文（含 satp，支持跨地址空间的 Trap 切换）
    pub context: ForeignContext,
    /// 进程的独立地址空间
    pub address_space: AddressSpace<Sv39, Sv39Manager>,
    /// 堆底地址
    pub heap_bottom: usize,
    /// 当前程序 break 位置（堆顶）
    pub program_brk: usize,
    /// Per-process syscall counters used by the `trace` exercise syscall.
    pub syscall_count: [usize; 512],
}

impl Process {
    /// 从 ELF 文件创建新进程。
    ///
    /// 步骤：
    /// 1. 验证 ELF 头：必须是 RISC-V 64 位可执行文件
    /// 2. 创建空的地址空间
    /// 3. 解析 ELF 的 LOAD 段，映射到地址空间（带权限标志）
    /// 4. 分配用户栈（2 页 = 8 KiB），映射到高地址区域
    /// 5. 创建 ForeignContext，设置入口地址和 satp
    pub fn new(elf: &[u8]) -> Option<Self> {
        // 验证 ELF 头：必须是 RISC-V 64 位可执行文件
        let entry = elf64_entry(elf)?;
        let phoff = read_u64(elf, 32)? as usize;
        let phentsize = read_u16(elf, 54)? as usize;
        let phnum = read_u16(elf, 56)? as usize;

        const PAGE_SIZE: usize = 1 << Sv39::PAGE_BITS;
        const PAGE_MASK: usize = PAGE_SIZE - 1;

        let mut address_space = AddressSpace::new();
        let mut max_end_va: usize = 0;

        // 遍历 ELF 的 LOAD 段，映射到地址空间
        let mut i = 0;
        while i < phnum {
            let ph = phoff + i * phentsize;
            i += 1;

            let p_type = read_u32(elf, ph)?;
            if p_type != PT_LOAD {
                continue;
            }
            let p_flags = read_u32(elf, ph + 4)?;
            let off_file = read_u64(elf, ph + 8)? as usize;
            let off_mem = read_u64(elf, ph + 16)? as usize;
            let len_file = read_u64(elf, ph + 32)? as usize;
            let len_mem = read_u64(elf, ph + 40)? as usize;

            let end_mem = off_mem + len_mem; // 虚拟地址结束
            assert_eq!(off_file & PAGE_MASK, off_mem & PAGE_MASK);

            // 记录最高虚拟地址（用于确定堆底）
            if end_mem > max_end_va {
                max_end_va = end_mem;
            }

            // 根据 ELF 段的权限标志构建页表项权限
            // U = 用户态可访问（必须设置）
            let mut flags: [u8; 5] = *b"U___V";
            if p_flags & PF_X != 0 {
                flags[1] = b'X';
            }
            if p_flags & PF_W != 0 {
                flags[2] = b'W';
            }
            if p_flags & PF_R != 0 {
                flags[3] = b'R';
            }
            // 将 ELF 段的数据映射到地址空间
            address_space.map(
                VAddr::new(off_mem).floor()..VAddr::new(end_mem).ceil(),
                elf.get(off_file..off_file + len_file)?,
                off_mem & PAGE_MASK,
                parse_flags(unsafe { core::str::from_utf8_unchecked(&flags) }).unwrap(),
            );
        }

        // 堆底从 ELF 加载的最高地址的下一页开始
        let heap_bottom = VAddr::<Sv39>::new(max_end_va).ceil().base().val();

        // 分配用户栈：2 页 = 8 KiB，映射到虚拟地址空间的高地址区域
        let stack = unsafe {
            alloc_zeroed(Layout::from_size_align_unchecked(
                2 << Sv39::PAGE_BITS,
                1 << Sv39::PAGE_BITS,
            ))
        };
        // 用户栈映射到 VPN [(1<<26)-2, 1<<26)，即虚拟地址空间的高区域
        address_space.map_extern(
            VPN::new((1 << 26) - 2)..VPN::new(1 << 26),
            PPN::new(stack as usize >> Sv39::PAGE_BITS),
            build_flags("U_WRV"), // 用户态可读写
        );

        // 创建用户态上下文
        let mut context = LocalContext::user(entry);
        // 构造 satp 值：MODE=8 (Sv39) | root_ppn
        let satp = (8 << 60) | address_space.root_ppn().val();
        // 用户栈顶指针（虚拟地址）
        *context.sp_mut() = 1 << 38;
        Some(Self {
            context: ForeignContext { context, satp },
            address_space,
            heap_bottom,
            program_brk: heap_bottom,
            syscall_count: [0; 512],
        })
    }

    /// 修改程序 break 位置（实现 sbrk 系统调用）。
    ///
    /// - `size > 0`：扩展堆，必要时映射新的物理页面
    /// - `size < 0`：收缩堆，必要时取消映射物理页面
    /// - 返回旧的 break 地址，失败返回 None
    pub fn change_program_brk(&mut self, size: isize) -> Option<usize> {
        let old_brk = self.program_brk;
        let new_brk = self.program_brk as isize + size;
        // 不允许堆顶低于堆底
        if new_brk < self.heap_bottom as isize {
            return None;
        }
        let new_brk = new_brk as usize;

        // 计算旧/新 break 所在页的上界（向上对齐到页边界）
        let old_brk_ceil = VAddr::<Sv39>::new(old_brk).ceil();
        let new_brk_ceil = VAddr::<Sv39>::new(new_brk).ceil();

        if size > 0 {
            // 扩展堆：映射新页面
            if new_brk_ceil.val() > old_brk_ceil.val() {
                self.address_space
                    .map(old_brk_ceil..new_brk_ceil, &[], 0, build_flags("U_WRV"));
            }
        } else if size < 0 {
            // 收缩堆：取消映射多余页面
            if old_brk_ceil.val() > new_brk_ceil.val() {
                self.address_space.unmap(new_brk_ceil..old_brk_ceil);
            }
        }

        self.program_brk = new_brk;
        Some(old_brk)
    }
}
