# rCore 第一章学习复盘与问答底稿

> 说明：本文是基于我和 AI 协作学习 rCore 第一章时的问答、纠错和本地实验过程整理出来的底稿。后续我会结合自己的理解重新手写/改写成正式学习笔记。

## 1. 第一章总体目标

rCore 第一章的核心目标不是实现完整操作系统，而是从一个普通 Rust 程序逐步退回到裸机环境，最后构建一个最小内核执行环境。

这一章主要回答几个问题：

- 普通 `Hello, world!` 程序为什么能在 Linux/Windows 上运行？
- 裸机平台为什么不能直接使用 `std` 和 `println!`？
- 没有标准库和 `main` 函数后，程序入口从哪里来？
- QEMU/RustSBI/内核之间如何交接控制权？
- 链接脚本为什么要把内核入口放到固定地址？
- 如何从终端字符输出扩展到 framebuffer 图形输出？

第一章可以看成一个“最小启动链”：

```text
源代码
  -> Rust 编译
  -> 链接脚本安排内存布局
  -> 生成内核 ELF
  -> QEMU 加载内核
  -> CPU 从入口指令开始执行
  -> SBI 输出字符或访问设备
```

## 2. 普通 Rust 程序为什么能 `println!`

普通 Rust 程序：

```rust
fn main() {
    println!("Hello, world!");
}
```

在 Windows/Linux 上可以直接运行，是因为它背后有完整的软件栈：

```text
应用程序
  -> Rust std 标准库
  -> C runtime / libc / 系统库
  -> 操作系统系统调用
  -> 设备驱动
  -> 硬件
```

`println!` 看起来只是一行代码，但实际依赖：

- 标准输出抽象；
- 操作系统提供的写文件/写终端系统调用；
- 程序启动时的运行时初始化；
- 进程、栈、堆等执行环境。

所以在裸机环境里，`println!` 不能直接使用。

## 3. 目标三元组与裸机平台

普通 Windows/Linux Rust 程序的目标三元组类似：

```text
x86_64-pc-windows-msvc
x86_64-unknown-linux-gnu
```

这里包含：

```text
CPU 架构 + 厂商/平台 + 操作系统 + 运行时 ABI
```

rCore 使用：

```text
riscv64gc-unknown-none-elf
```

含义：

```text
riscv64gc：RISC-V 64 位架构，带通用扩展
unknown：厂商未知
none：没有操作系统
elf：生成 ELF 格式可执行文件
```

关键在于 `none`：

```text
没有操作系统
没有系统调用
没有 std 标准库依赖的运行时环境
```

因此必须使用：

```rust
#![no_std]
```

## 4. 我对 no_std/core/std 的理解与修正

### 问题

为什么换成 `riscv64gc-unknown-none-elf` 后不能使用 `std`？

### 我的回答

Linux/Windows 已经有操作系统，所以普通 `cargo run` 可以调用底层系统功能。`none` 表示没有操作系统环境，因此找不到 `std`。

`core` 是 Rust 最基础的核心库，不依赖操作系统。

### 修正后的答案

这个理解基本正确。更精确地说：

```text
std = core + alloc + OS 相关能力
```

`std` 依赖：

- 线程；
- 文件；
- 标准输入输出；
- 堆分配；
- 系统调用；
- panic 打印和退出。

裸机平台没有操作系统支持，所以无法使用 `std`。

`core` 则只包含不依赖操作系统的语言核心能力，例如：

- 基本类型；
- Option / Result；
- trait；
- slice；
- volatile；
- core::ptr；
- core::fmt 的格式化框架。

但 `core::fmt` 只负责“格式化字符串”，不负责“把字符串输出到哪里”。所以后续我们自己实现了一个 writer，把字符送到 SBI 或 UART。

## 5. 为什么要移除 `println!`

`println!` 来自标准库，需要标准输出和系统调用。

裸机没有 stdout，也没有操作系统，所以第一步必须移除：

```rust
println!("Hello, world!");
```

如果加上：

```rust
#![no_std]
```

编译器会报错：

```text
cannot find macro `println` in this scope
```

这是合理的，因为 `println!` 不在 `core` 里。

## 6. 为什么需要 panic_handler

加了 `#![no_std]` 后，编译器会要求：

```rust
#[panic_handler]
```

原因是 Rust 中很多操作可能 panic：

- `assert!` 失败；
- `unwrap()` 遇到 `None` 或 `Err`；
- 数组越界；
- 主动调用 `panic!`。

在 `std` 环境中，panic 由标准库处理。

裸机没有标准库，所以必须自己提供：

```rust
use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

这里返回类型是：

```rust
!
```

表示这个函数永远不返回。

## 7. 为什么要 `#![no_main]`

普通 Rust 程序从 `main()` 开始，但真实执行并不是 CPU 直接跳到 `main`。

普通程序启动链大概是：

```text
操作系统加载程序
  -> C runtime / Rust runtime 初始化
  -> 设置栈、参数、环境变量
  -> 调用 main()
```

裸机没有运行时，因此不能依赖普通 `main`。

所以需要：

```rust
#![no_main]
```

意思是：

```text
不要使用 Rust 标准 main 入口，我自己定义真正入口。
```

## 8. 我对移除 main 的理解与修正

### 问题

为什么删掉 `main` 后程序变成空程序？

### 我的回答

因为现在没有程序入口，类似只有整体框架，没有第一条真正执行的指令。

### 修正后的答案

这个理解正确。

加上：

```rust
#![no_main]
```

又没有提供任何入口符号时，编译器虽然可以生成一个 ELF 文件，但里面没有有效入口逻辑。`rust-readobj` 看到入口可能是 0，反汇编也可能没有有效指令。

所以后续必须自己提供入口 `_start`。

## 9. QEMU/RustSBI/内核启动流程

QEMU 模拟 RISC-V 机器时，启动流程大致是：

```text
QEMU 固化小程序
  -> RustSBI / bootloader
  -> rCore 内核
```

典型地址：

```text
0x1000      QEMU 固化启动代码
0x80000000  RustSBI / M 态固件
0x80200000  rCore 内核入口
```

流程：

```text
QEMU CPU PC = 0x1000
  -> 跳到 0x80000000
  -> RustSBI 初始化机器
  -> 跳到 0x80200000
  -> 内核第一条指令
```

在 tg 组件化版本里，`tg-sbi` 可以通过 `nobios` feature 内置最小 SBI，因此 QEMU 参数可以使用：

```text
-bios none
```

## 10. 我对 QEMU 的理解与修正

### 我的回答

QEMU 像是在本机上模拟一个没装系统的新计算机，CPU、内存等都是它模拟出来的。它把 bootloader 和内核放到指定物理地址，然后按照启动流程跳转。

### 修正后的答案

这个理解基本正确。

更准确地说：

```text
QEMU 是一个用户态程序
但它模拟了一台 RISC-V 计算机
包括 CPU、物理内存、UART、VirtIO 设备等
```

宿主机 Windows 上看到的是一个普通进程：

```text
qemu-system-riscv64.exe
```

但在这个进程内部，guest 看到的是：

```text
RISC-V CPU
物理地址空间
MMIO 外设
```

## 11. 链接脚本 linker.ld 的作用

链接脚本告诉链接器：

- 程序入口是谁；
- `.text`、`.rodata`、`.data`、`.bss` 放在哪里；
- 内核应该从哪个物理地址开始布局；
- 哪些段要保留，哪些段要丢弃。

典型设置：

```ld
OUTPUT_ARCH(riscv)
ENTRY(_start)
BASE_ADDRESS = 0x80200000;
```

含义：

```text
目标架构是 RISC-V
入口符号是 _start
内核起始地址是 0x80200000
```

为什么必须匹配？

```text
RustSBI 会跳到 0x80200000
如果内核第一条指令不在这里
CPU 就会跳到错误内容
```

这和 STM32 的中断向量表、链接地址不匹配会跑飞类似。

## 12. ELF 和 bin 镜像

ELF 文件包含：

- 代码段；
- 数据段；
- 符号表；
- 段表；
- 调试信息；
- 元数据。

QEMU 简单 loader 有时只是把文件逐字节拷贝到内存，不理解 ELF 元数据。

所以教程中会用：

```bash
rust-objcopy --strip-all os -O binary os.bin
```

把 ELF 里的有效代码/数据提取成纯二进制镜像。

在新版 QEMU 或当前 tg 框架中，有时可以直接加载 ELF，但理解 ELF/bin 区别仍然重要。

## 13. `_start`、`entry.asm` 与 Rust 入口

传统教程里会写：

```asm
.section .text.entry
.globl _start
_start:
    li x1, 100
```

然后在 Rust 里：

```rust
global_asm!(include_str!("entry.asm"));
```

在 tg ch1 中，入口可能直接写成 Rust naked function：

```rust
#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
unsafe extern "C" fn _start() -> ! {
    const STACK_SIZE: usize = 4096;
    static mut STACK: [u8; STACK_SIZE] = [0u8; STACK_SIZE];

    core::arch::naked_asm!(
        "la sp, {stack} + {stack_size}",
        "j  {main}",
        stack_size = const STACK_SIZE,
        stack = sym STACK,
        main = sym rust_main,
    )
}
```

作用：

```text
1. 设置栈指针 sp
2. 跳转到 Rust 主函数 rust_main
```

## 14. 第一章最小输出链路

原始 ch1 的输出链：

```text
rust_main()
  -> for c in b"Hello, world!\n"
  -> tg_sbi::console_putchar(c)
  -> SBI legacy console_putchar
  -> QEMU UART/终端输出
```

代码：

```rust
extern "C" fn rust_main() -> ! {
    for c in b"Hello, world!\n" {
        console_putchar(*c);
    }
    shutdown(false)
}
```

这里 `shutdown(false)` 会正常关机退出 QEMU。

## 15. 本地环境配置复盘

本地 Windows 环境最终需要：

- Git；
- Rustup；
- Rust stable；
- `riscv64gc-unknown-none-elf` target；
- QEMU；
- Visual Studio Build Tools / MSVC linker；
- Windows SDK；
- 可选 `cargo-binutils`。

本地遇到的问题：

### 15.1 rustup 下载慢

解决方式：使用 rsproxy 镜像。

配置文件：

```text
C:\Users\FLY\.cargo\config.toml
```

内容：

```toml
[source.crates-io]
replace-with = "rsproxy-sparse"

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"

[net]
git-fetch-with-cli = true
```

### 15.2 缺少 `link.exe`

报错：

```text
error: linker `link.exe` not found
```

原因：

```text
Windows MSVC Rust 工具链需要 MSVC linker。
VS Code 不是 Visual Studio Build Tools。
```

解决：

```text
安装 Visual Studio Build Tools C++ 工具链
```

### 15.3 缺少 `kernel32.lib`

报错：

```text
LINK : fatal error LNK1181: 无法打开输入文件“kernel32.lib”
```

原因：

```text
Windows SDK 没装完整。
```

解决：

```text
等待 Windows SDK 安装完成。
```

### 15.4 本地环境脚本

为了每次不用手动设置 PATH/LIB，创建：

```text
C:\Users\FLY\Desktop\OS\setup-rcore-env.ps1
```

使用方式：

```powershell
. C:\Users\FLY\Desktop\OS\setup-rcore-env.ps1
```

然后进入 ch1：

```powershell
cd C:\Users\FLY\Desktop\OS\tg-rcore-tutorial-test\tg-rcore-tutorial-ch1
cargo run
```

## 16. ch1-tangram 图形化实验目标

老师 demo 是基于 ch1 扩展：

```text
在 ch1 最小内核基础上
通过 VirtIO-GPU 访问 framebuffer
把七巧板 “OS” 图案渲染到 QEMU 图形窗口
```

我们的目标：

```text
Hello world 终端输出
  -> framebuffer 彩色像素输出
  -> 七巧板 OS 图案
```

## 17. 图形化实验第一阶段：Canvas 抽象

最开始没有直接写 GPU，而是先做抽象：

```rust
pub trait Canvas {
    fn put_pixel(&mut self, x: usize, y: usize, color: Color);
}
```

意义：

```text
绘图算法不关心像素最终去哪里
可以先输出到 MemoryCanvas
后面替换成 FramebufferCanvas
```

类比后端：

```text
Controller/Service 不直接关心底层 TCP/MySQL 协议
而是调用抽象接口
```

## 18. MemoryCanvas 与 framebuffer 思想

MemoryCanvas 使用数组模拟屏幕：

```rust
pixels: [u32; WIDTH * HEIGHT]
```

二维坐标映射到一维数组：

```rust
let index = y * WIDTH + x;
```

例如：

```text
(0, 0) -> 0
(1, 0) -> 1
(0, 1) -> WIDTH
```

这就是 framebuffer 的核心思想：

```text
屏幕 = 一块连续内存
像素 = 内存里的一个元素
```

## 19. 为什么局部大数组会导致栈爆

我们曾经在 `demo()` 里创建：

```rust
let mut canvas = MemoryCanvas::new();
```

如果 `MemoryCanvas` 里有：

```rust
pixels: [u32; 160 * 90]
```

大约需要：

```text
160 * 90 * 4 = 57600 字节
```

但 ch1 原始内核栈只有：

```rust
const STACK_SIZE: usize = 4096;
```

因此可能栈爆，导致只看到：

```text
Hello, world!
```

后续代码不执行。

解决：

```rust
const STACK_SIZE: usize = 131072;
```

这也是内核/嵌入式常见问题：

```text
不要随便把大数组放在栈上。
```

## 20. ASCII framebuffer 预览阶段

由于 CNB 云端无法直接通过 HTTP 打开 VNC 协议，我们做过 ASCII 预览：

```text
非零像素 -> '#'
零像素   -> ' '
```

这验证了：

```text
图形数据
  -> draw_rect/draw_polygon
  -> put_pixel
  -> MemoryCanvas
```

虽然不是最终彩色图，但证明了绘图算法能工作。

## 21. 本地 VirtIO-GPU 彩色图形化

本地环境配好后，使用 QEMU 图形窗口：

```toml
runner = [
    "qemu-system-riscv64",
    "-machine",
    "virt",
    "-display",
    "gtk",
    "-serial",
    "stdio",
    "-device",
    "virtio-gpu-device,bus=virtio-mmio-bus.0,xres=800,yres=480",
    "-D",
    "qemu.log",
    "-bios",
    "none",
    "-kernel",
]
```

注意：

- `sdl` 后端在本机曾崩溃；
- `gtk` 后端可用；
- `-serial stdio` 用于终端日志；
- `virtio-gpu-device` 提供虚拟显卡；
- `xres=800,yres=480` 显式设置分辨率。

## 22. VirtIO-GPU 初始化链

图形代码核心流程：

```text
graphics::demo()
  -> MmioTransport::new(0x1000_1000)
  -> VirtIOGpu::new(transport)
  -> gpu.resolution()
  -> gpu.setup_framebuffer()
  -> framebuffer.fill(0)
  -> FramebufferCanvas::new(...)
  -> draw_os_logo(...)
  -> gpu.flush()
```

对应含义：

```text
MmioTransport：
  通过 MMIO 地址访问 VirtIO 设备

VirtIOGpu：
  virtio-drivers 提供的 GPU 驱动

setup_framebuffer：
  创建 2D resource
  分配 framebuffer DMA
  attach backing
  set scanout

flush：
  transfer_to_host_2d
  resource_flush
  让 QEMU 窗口显示最新像素
```

## 23. 为什么需要 allocator

加入 `virtio-drivers` 后出现：

```text
no global memory allocator found
```

原因：

```text
virtio-drivers 内部需要动态分配 DMA/队列等结构
ch1 原本没有堆分配器
```

解决：加入一个最小 bump allocator。

特点：

```text
只分配
不释放
适合 ch1 这种启动后画一次图的 demo
```

## 24. 为什么 setup_framebuffer 一开始失败

一开始日志：

```text
[graphics] init virtio transport
[graphics] init virtio gpu
[graphics] get resolution
[graphics] setup framebuffer
[graphics] failed to setup framebuffer
```

原因是 DMA 池太小。

原来：

```rust
const DMA_PAGES: usize = 64;
```

也就是：

```text
64 * 4096 = 256 KiB
```

但 framebuffer 需要：

```text
800 * 480 * 4 = 1,536,000 字节，约 1.5 MiB
```

所以必须扩大：

```rust
const DMA_PAGES: usize = 512;
```

约：

```text
512 * 4096 = 2 MiB
```

这样 `setup_framebuffer()` 才能成功。

## 25. 为什么需要 kill QEMU

图形 demo 最后会：

```rust
loop {
    core::hint::spin_loop();
}
```

目的是保持 QEMU 窗口不关闭。

因此 `cargo run` 不会自动结束。

如果要重新运行，先杀掉旧 QEMU：

```powershell
Get-Process qemu-system-riscv64 -ErrorAction SilentlyContinue | Stop-Process -Force
```

或：

```powershell
taskkill /IM qemu-system-riscv64.exe /F
```

## 26. 从矩形 logo 到七巧板 logo

最初图案只是矩形块：

```text
O/S 的粗略块状 logo
```

后来发现老师 demo 是七巧板风格：

```text
多个彩色三角形、平行四边形拼出 OS
```

因此修改为多边形绘制：

```rust
struct PolyPoint {
    x: isize,
    y: isize,
}

struct Polygon {
    points: [PolyPoint; 4],
    len: usize,
    color: Color,
}
```

绘制逻辑：

```text
1. 计算多边形包围盒
2. 遍历包围盒内每个像素
3. 判断像素是否在多边形内
4. 如果在，则 put_pixel
```

判断点是否在凸多边形内：

```rust
fn edge(a: PolyPoint, b: PolyPoint, p: PolyPoint) -> isize {
    (p.x - a.x) * (b.y - a.y) - (p.y - a.y) * (b.x - a.x)
}
```

如果一个点在凸多边形所有边的同一侧，则认为在内部。

## 27. 当前 ch1-tangram 调用链

```text
main.rs::_start
  -> 设置 sp
  -> 跳到 rust_main

main.rs::rust_main
  -> 输出 Hello, world!
  -> graphics::demo()
  -> loop 保持窗口

graphics.rs::demo
  -> 初始化 VirtIO transport
  -> 初始化 VirtIOGpu
  -> 获取分辨率
  -> setup_framebuffer
  -> framebuffer.fill(0)
  -> FramebufferCanvas::new
  -> draw_os_logo
  -> gpu.flush
```

图形绘制链：

```text
draw_os_logo
  -> triangle / quad 创建 Polygon
  -> draw_polygon
  -> contains 判断点是否在多边形内
  -> FramebufferCanvas::put_pixel
  -> 写 framebuffer[index]
```

像素写入：

```rust
let index = (y * self.width + x) * 4;
self.framebuffer[index] = color.b;
self.framebuffer[index + 1] = color.g;
self.framebuffer[index + 2] = color.r;
self.framebuffer[index + 3] = 0xff;
```

这里格式是 BGRA。

## 28. 第一章关键问答汇总

### Q1：为什么裸机不能用 `println!`？

我的初始理解：

```text
println! 依赖 std，而 std 依赖操作系统。
```

修正答案：

```text
println! 不只是格式化，还要把数据写到标准输出。
标准输出依赖操作系统文件描述符/系统调用。
裸机没有 OS，所以不能直接用 println!。
```

### Q2：为什么需要 `panic_handler`？

修正答案：

```text
Rust 编译器要求 panic! 有明确处理方式。
std 环境由标准库提供。
no_std 环境必须自己实现。
```

### Q3：为什么需要 `no_main`？

修正答案：

```text
普通 main 依赖运行时初始化。
裸机没有运行时，所以禁用普通 main，自己定义 _start。
```

### Q4：`_start` 和 `rust_main` 的关系是什么？

修正答案：

```text
_start 是 CPU/链接脚本意义上的真正入口。
它先设置栈 sp，然后跳到 Rust 函数 rust_main。
rust_main 才是我们写主要逻辑的地方。
```

### Q5：为什么链接地址要是 `0x80200000`？

修正答案：

```text
QEMU/RustSBI 约定把控制权交给 0x80200000。
内核第一条指令必须被链接并加载到这个地址。
否则 CPU 会跳到错误内容。
```

### Q6：ELF 为什么不能总是直接给 QEMU？

修正答案：

```text
ELF 包含元数据。
某些简单 loader 只会逐字节复制，不理解 ELF。
因此教程中常用 objcopy 提取纯二进制镜像。
```

### Q7：Framebuffer 本质是什么？

修正答案：

```text
Framebuffer 本质是一段连续内存。
屏幕上的每个像素对应内存中的若干字节。
二维坐标通过 y * width + x 映射到一维地址。
```

### Q8：为什么 MemoryCanvas 会导致栈问题？

修正答案：

```text
MemoryCanvas 里有大数组。
如果作为局部变量创建，会放在栈上。
ch1 原始栈只有 4 KiB，大数组会栈爆。
```

### Q9：为什么 `setup_framebuffer` 失败？

修正答案：

```text
VirtIO-GPU framebuffer 需要 DMA 内存。
800x480x4 约 1.5 MiB。
原来 DMA 池只有 256 KiB，所以分配失败。
扩大 DMA 池后解决。
```

### Q10：为什么 QEMU 窗口不会自动退出？

修正答案：

```text
为了保留图形画面，rust_main 调用 graphics::demo 后进入 loop。
因此 cargo run 会一直运行，必须手动 Ctrl+C 或 kill QEMU。
```

## 29. 第一章我现在应该掌握的核心链路

```text
编译期：
  Cargo.toml / .cargo/config.toml
  -> rustc 交叉编译到 riscv64gc-unknown-none-elf
  -> linker.ld 安排入口和内存布局

启动期：
  QEMU
  -> tg-sbi / M 态入口
  -> _start
  -> 设置 sp
  -> rust_main

字符输出：
  rust_main
  -> console_putchar
  -> SBI
  -> UART/终端

图形输出：
  rust_main
  -> graphics::demo
  -> VirtIOGpu
  -> setup_framebuffer
  -> draw_os_logo
  -> framebuffer 像素写入
  -> gpu.flush
  -> QEMU GTK 窗口显示
```

## 30. 后续正式笔记可以怎么写

正式笔记建议按下面结构：

```text
1. 普通应用程序为什么能运行
2. 裸机平台与 no_std
3. panic_handler / no_main / _start
4. QEMU/RustSBI/内核启动地址
5. 链接脚本与 ELF/bin
6. ch1 原始 Hello world 调用链
7. ch1-tangram 图形扩展设计
8. Canvas 抽象与 framebuffer 原理
9. VirtIO-GPU 初始化流程
10. 调试记录：link.exe、kernel32.lib、DMA 不足、QEMU 后端
11. 当前成果与后续改进
```

## 31. 当前成果总结

目前已经完成：

- 本地 Rust/QEMU/Build Tools 环境配置；
- ch1 原始 `Hello, world!` 本地运行；
- ch1 扩展 VirtIO-GPU；
- framebuffer 彩色输出；
- 多边形七巧板风格 OS 图案；
- QEMU GTK 图形窗口显示。

当前实现还比较简单：

- allocator 是一次性 bump allocator；
- GPU 驱动直接放在 ch1 中，未组件化；
- 图案坐标是手写多边形；
- 没有动画；
- 没有真正和老师 demo 一模一样。

但它已经体现了 ch1-tangram 的关键学习目标：

```text
从最小裸机 Hello world 出发，
扩展到 framebuffer 图形显示，
理解裸机程序、MMIO、VirtIO-GPU 和像素绘制的基本链路。
```
