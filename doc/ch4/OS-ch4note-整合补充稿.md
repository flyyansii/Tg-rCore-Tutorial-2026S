# OS ch4 地址空间与 Tetris 实验整合补充稿

## 一、本章总览

第四章的主题是地址空间。前面章节中，操作系统已经可以加载应用、处理系统调用、进行任务切换，但用户程序和内核之间的内存隔离还不完整。ch4 引入 Sv39 页表机制，为每个用户程序建立独立地址空间。

本章可以用一句话概括：

```text
内核不再把用户程序当成“物理内存中的一段代码”，而是为每个程序构造一个“虚拟内存世界”。
```

在这个虚拟内存世界中，用户程序看到的是虚拟地址；真实物理内存在哪里，由页表决定；页表由内核创建；地址翻译由 CPU 的 MMU 自动完成。

## 二、从物理地址到虚拟地址

物理地址是真实内存地址。CPU 最终访问内存时，必须落到物理地址。

虚拟地址是程序看到的地址。用户程序以为自己在访问某个连续地址空间，但这些地址未必对应连续物理内存。

页表负责记录：

```text
虚拟页号 VPN -> 物理页号 PPN
```

页表项除了记录 PPN，还记录权限，例如：

- `R`：可读。
- `W`：可写。
- `X`：可执行。
- `U`：用户态可访问。
- `V`：页表项有效。

如果用户程序访问没有映射的虚拟地址，或者权限不满足，就会触发 PageFault。

## 三、Sv39 和 satp

Sv39 是 RISC-V 64 位架构的一种分页方案。它使用三级页表。虚拟地址会被拆成三级 VPN 和页内偏移：

```text
VPN[2] | VPN[1] | VPN[0] | offset
```

查表过程：

```text
satp 保存根页表 PPN
  -> MMU 用 VPN[2] 查第三级页表
  -> 用 VPN[1] 查第二级页表
  -> 用 VPN[0] 查第一级页表
  -> 得到最终 PPN
  -> PPN + offset 得到物理地址
```

`satp` 是地址空间切换的关键。每个进程的地址空间都有自己的根页表。切换进程时，只要切换 `satp`，CPU 后续看到的虚拟地址解释方式就变了。

## 四、内核如何加载用户程序

ch4 中用户程序以 ELF 形式被打包进内核镜像。内核启动后通过 `AppMeta::locate()` 找到用户程序数据，然后调用 `Process::new()` 创建进程。

`Process::new()` 的核心任务：

```text
读取 ELF 入口地址
读取 Program Header
遍历 PT_LOAD 段
根据段权限建立页表映射
复制代码和数据
映射用户栈
构造 ForeignContext
保存 satp
```

可以理解为：ELF 文件只是磁盘/镜像中的程序格式；`Process::new()` 才真正把它变成一个可运行的用户进程。

## 五、系统调用中的地址翻译

ch4 最容易出错的点是用户指针。

例如用户程序调用：

```rust
write(fd, buf, count)
```

`buf` 是用户虚拟地址。内核不能直接解引用它。正确流程是：

```text
用户态传入 buf
  -> ecall 进入内核
  -> 内核找到当前进程
  -> 访问 process.address_space
  -> translate(buf, 权限)
  -> 得到内核可访问指针
  -> 再读写数据
```

这就是为什么 ch4 的 `write/read/clock_gettime/trace` 等系统调用都要增加地址翻译逻辑。

## 六、mmap、munmap、sbrk

这三个系统调用都和页表有关。

`mmap` 用于建立映射：

```text
在某个虚拟地址范围内分配物理页，并设置权限。
```

`munmap` 用于取消映射：

```text
把某段虚拟地址对应的页表项清空。
```

`sbrk` 用于调整堆：

```text
扩大堆时映射新页。
缩小堆时撤销映射。
```

因此，这些系统调用不是简单“申请数组”，而是在修改当前进程的地址空间。

## 七、ch4 Tetris 扩展实验

本次扩展实现了用户态单人俄罗斯方块游戏。实现目标：

- 支持方块移动。
- 支持方块旋转。
- 支持自动下落。
- 支持硬降。
- 支持行消除。
- 支持计分。
- 支持速度递增。
- 使用 QEMU GTK 窗口图形显示。
- 使用 VirtIO-keyboard 输入。

设计思路是模仿 ch3 snake 的用户态游戏路线，但放到 ch4 地址空间环境中。

## 八、用户态 Tetris 程序

用户程序文件：

```text
tg-rcore-tutorial-user/src/bin/ch4_tetris.rs
```

用户态负责游戏规则：

```text
生成方块
计算旋转
检测碰撞
固定方块
消除满行
更新分数
生成一帧棋盘数据
```

用户态不直接操作 GPU。它只把当前棋盘打包成 `TetrisFrame`，然后：

```rust
write(GRAPHICS_FD, frame_bytes)
```

其中 `GRAPHICS_FD = 3` 是本实验约定的图形输出 fd。

## 九、内核图形输出

内核图形文件：

```text
tg-rcore-tutorial-ch4/src/graphics.rs
```

内核收到 `write(fd=3)` 后：

```text
检查 fd 是否为 GRAPHICS_FD
翻译用户态 frame 指针
检查 magic 和长度
初始化 VirtIO-GPU
把 10x20 棋盘画成彩色块
flush framebuffer
```

这里特别体现 ch4 原理：用户态传来的 `frame` 指针必须先翻译，不能直接用。

## 十、内核键盘输入

内核键盘文件：

```text
tg-rcore-tutorial-ch4/src/keyboard.rs
```

键盘输入通过 VirtIO-keyboard 获取。内核把键盘事件转成：

```text
a / d / w / s / space / q
```

用户程序仍然通过标准输入读取：

```rust
try_getchar()
```

这说明用户程序不需要知道底层键盘设备是什么，只要读标准输入即可。

## 十一、本次关键 bug 与修复

现象：

```text
QEMU 窗口打开了，但没有游戏画面。
```

定位过程：

1. 先确认 `cargo run` 是否真的打包了 `ch4_tetris`。
2. 发现默认 case 需要从原 ch4 测试切换到 `ch4_tetris`。
3. 修复后用户程序能启动，串口输出 Tetris 提示。
4. 随后出现 `LoadPageFault: stval = 0x10001000`。
5. `0x10001000` 是 VirtIO-GPU 的 MMIO 地址。

根因：

```text
ch4 开启页表后，内核地址空间没有映射 QEMU VirtIO 设备 MMIO 区。
```

修复：

```text
在 kernel_space() 中映射 0x1000_0000..0x1000_3000。
```

该区间覆盖：

```text
0x1000_0000 UART
0x1000_1000 VirtIO-GPU
0x1000_2000 VirtIO-keyboard
```

修复后出现：

```text
virtio-gpu ready
virtio-keyboard ready
```

说明图形和键盘设备初始化成功。

## 十二、运行方式

本地图形运行：

```powershell
cd C:\Users\FLY\Desktop\OS\tg-rcore-tutorial-test\tg-rcore-tutorial-ch4
cargo run
```

操作：

```text
a：左移
d：右移
w：旋转
s：加速下落
space：硬降
q：退出
```

基础测试：

```powershell
cargo run --features base
```

练习测试：

```powershell
cargo run --features exercise
```

## 十三、学习收获

这章最重要的收获是：地址空间并不是只在“内存管理章节”出现一次，而是会影响整个内核。

它影响：

- 程序加载。
- 进程上下文。
- 系统调用。
- 用户指针访问。
- 堆管理。
- mmap/munmap。
- 图形设备驱动。
- MMIO 设备访问。

本次 Tetris 实验尤其说明：当分页开启后，连内核访问设备地址都必须考虑页表映射。之前在 ch1/ch2 里可以直接访问 `0x1000_1000`，到了 ch4 就不行了，因为地址空间规则已经改变。

## 十四、后续改进方向

后续可以继续改进：

- 增加更漂亮的 UI 文本。
- 增加下一块方块预览。
- 增加暂停功能。
- 增加保存最高分。
- 把 GPU/keyboard 抽象成更通用的设备接口。
- 把图形帧协议整理成文档，方便后续 ch5/ch6 游戏复用。

## 十五、按 Guide 讲 ch4 的 30 步流程版

这一版用于把 ch4 主线串成可讲述流程。

1. ch2/ch3 已经有用户态、内核态和任务切换。
2. ch4 发现还缺一个关键问题：内存隔离。
3. 内核引入地址空间 `AddressSpace`。
4. 地址空间的核心数据结构是页表。
5. 页表把虚拟页 VPN 映射到物理页 PPN。
6. 页表项 PTE 记录有效位和权限位。
7. 内核先建立自己的内核地址空间。
8. 内核映射代码段、只读段、数据段和堆。
9. 内核映射 MMIO 设备地址。
10. 内核开启 Sv39 分页。
11. 用户程序以 ELF 形式被内核找到。
12. 内核解析 ELF 入口地址。
13. 内核解析 Program Header。
14. 内核挑出 LOAD 段。
15. 内核为 LOAD 段计算虚拟页范围。
16. 内核根据 ELF 权限设置页表权限。
17. 内核分配物理页帧。
18. 内核把 ELF 段复制到物理页。
19. 内核建立用户虚拟页到物理页的映射。
20. 内核清零 `.bss`。
21. 内核映射用户栈。
22. 内核生成进程对应的 `satp`。
23. 进入用户态前，CPU 使用该进程页表。
24. 用户程序访问虚拟地址时，MMU 自动翻译。
25. 用户程序 syscall 传来的指针仍然是虚拟地址。
26. 内核通过当前进程 AddressSpace 翻译该指针。
27. 权限不对时，内核拒绝或触发异常处理。
28. `mmap/munmap/sbrk` 都是在修改页表和地址空间范围。
29. Tetris 图形帧通过用户虚拟地址传入内核，并经翻译后绘制。
30. ch4 最终把“程序运行”升级为“进程在独立地址空间中运行”。

## 十六、ch4 和前后章节的连接

```text
ch3 之前：
任务切换主要关心上下文和栈。

ch4 之后：
任务切换还必须关心地址空间和 satp。
```

ch4 之后我再看 syscall，会多一层意识：

```text
用户传来的所有地址都不是内核能直接用的地址。
```

这对后续文件系统、进程、管道都很重要，因为它们都会在用户态和内核态之间传递 buffer。

## 十七、Guide 代码树对照讲解

Guide ch4 的 `mm/` 目录其实是在回答四个问题：

1. 地址如何表示？
2. 物理页如何分配？
3. 页表如何建立？
4. 一个进程的完整地址空间如何管理？

对应到代码树：

```text
mm/address.rs
  -> 解决“地址如何表示”
  -> VirtAddr、PhysAddr、VPN、PPN、页内偏移

mm/frame_allocator.rs
  -> 解决“物理页从哪里来”
  -> 分配和回收物理页帧

mm/page_table.rs
  -> 解决“单个虚拟页如何映射到物理页”
  -> PTE、map、unmap、translate

mm/memory_set.rs
  -> 解决“一个进程完整地址空间由哪些区域组成”
  -> ELF 段、用户栈、TrapContext、mmap、heap
```

组件化版本的对应关系：

```text
Guide mm/address.rs + page_table.rs + memory_set.rs
  -> tg-rcore-tutorial-kernel-vm

Guide frame_allocator.rs + heap_allocator.rs
  -> tg-rcore-tutorial-kernel-alloc

Guide loader.rs
  -> build.rs + AppMeta

Guide task 中的 memory_set 字段
  -> ch4/process.rs 中 Process 保存 AddressSpace

Guide syscall 中的 mmap/munmap/sbrk/read/write
  -> ch4/main.rs 中 syscall trait 实现
```

我读组件化仓库时，不能因为 `mm` 目录不在 ch4/src 里就以为没有内存管理。它只是被抽到了 `tg-kernel-vm` 这个 crate 中。

## 十八、ch4 模块何时被调用

```text
内核刚启动：
  main.rs 初始化 console/log/heap

准备开启分页：
  main.rs::kernel_space
  -> tg-kernel-vm 创建内核 AddressSpace
  -> 映射内核段和 MMIO

准备运行用户程序：
  AppMeta 找到 ELF 字节
  -> process.rs::Process::new
  -> 解析 ELF
  -> tg-kernel-vm 建立用户 AddressSpace

用户程序执行：
  CPU 根据 satp 使用当前进程页表

用户发起 syscall：
  main.rs syscall impl
  -> 找当前 Process
  -> Process.address_space.translate 用户指针

用户请求 mmap/munmap/sbrk：
  syscall impl 修改当前 Process 的 AddressSpace

Tetris 画图：
  用户 write(fd=3)
  -> translate frame 指针
  -> graphics.rs
  -> VirtIO-GPU MMIO
```

这就是 ch4 和 ch2/ch3 最大不同：前面主要保存“执行现场”，ch4 开始还要保存和使用“地址空间现场”。
