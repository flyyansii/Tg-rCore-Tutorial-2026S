# rCore ch6 文件系统整合补充稿

## 1. 这一章到底在解决什么问题

ch6 的主题是文件系统。前面几章已经让程序能启动、能被调度、能拥有独立地址空间、能用进程生命周期来管理，但这些程序大多还像“没有长期记忆的人”。程序退出后，内存里的数据就没了。

ch6 要解决的是：用户程序如何通过统一的文件接口，把数据读写到一个可以长期保存的对象中。

这里的“文件”不要只理解成磁盘上的文本文件。操作系统里常见的文件抽象包括：

- 控制台输入输出。
- 普通磁盘文件。
- 目录项。
- 块设备上的文件系统对象。
- 后面章节会出现的管道。
- 本实验里扩展出来的特殊图形 fd。

一句话概括：ch6 把“数据从用户程序到真实存储设备”的路径接起来了。

## 2. 从 ch5 到 ch6：为什么进程之后要有文件系统

ch5 里进程可以 fork、exec、wait、exit，看起来已经很像 Linux。但它还有一个明显缺口：进程之间和进程退出之后的数据如何保留？

如果只有内存：

- 程序运行时数据存在。
- 程序退出后数据消失。
- 父进程只能通过 wait 拿到退出码。
- 不同程序很难通过持久化数据协作。

引入文件系统后：

- 程序可以 open 一个文件。
- 程序可以 read/write 文件内容。
- 程序可以 close 文件。
- 程序可以 link/unlink 操作目录项。
- 内核可以把磁盘镜像当作块设备来保存数据。

这就是 ch6 的本质：进程获得了“长期存储”和“统一 I/O 接口”。

## 3. 文件描述符 fd 是什么

文件描述符可以先理解成“当前进程打开文件表里的下标”。

用户程序不会直接拿到内核里的 `File` 对象，也不会直接操作 inode。用户程序只拿到一个整数：

```text
fd = open("filea", flags)
write(fd, buf, len)
read(fd, buf, len)
close(fd)
```

这个 fd 的真正含义是：

```text
当前进程 Process
-> fd_table
-> fd_table[fd]
-> Arc<Mutex<OSInode>>
-> Inode
-> easy-fs
-> 块设备
```

所以 fd 不是全局编号。不同进程里的 `fd=3` 可以指向完全不同的对象。

## 4. 为什么 stdin/stdout 也能看成文件

Guide 里强调 Unix 的“一切皆文件”思想。

在 ch6 中，`fd=0` 通常表示标准输入，`fd=1` 通常表示标准输出。它们不一定对应磁盘文件，但它们也能通过 `read/write` 统一访问。

这就是文件抽象的厉害之处：

```text
write(1, "hello", 5)
-> 输出到控制台

write(fd_file, "hello", 5)
-> 写入磁盘文件

write(3, frame, len)
-> 本实验扩展：写入图形帧，交给 VirtIO-GPU 渲染
```

同一个系统调用接口，背后可以接不同设备或对象。

## 5. UserBuffer 为什么难理解

用户程序传给内核的 `buf` 是用户虚拟地址。

内核不能直接把这个地址当成内核地址用。原因是 ch4 已经引入了地址空间隔离：

```text
用户看到的 0x10000
不等于
内核能直接访问的 0x10000
```

所以内核拿到用户传来的 `buf` 后，要通过当前进程的页表翻译：

```text
用户虚拟地址
-> 当前进程 AddressSpace / MemorySet
-> 页表翻译
-> 真实物理页对应的内核可访问地址
```

在组件化 ch6 里，这个动作常见形式是：

```rust
current.address_space.translate::<u8>(VAddr::new(buf), READABLE)
```

这就是我们之前一直说的：系统调用不是简单函数调用，它跨过了用户地址空间和内核地址空间。

## 6. read/write 的基本流程

以 `write(fd, buf, len)` 为例：

```text
用户程序调用 write
-> user_lib::write
-> syscall 编号和参数放入寄存器
-> ecall 进入内核
-> trap 入口保存上下文
-> syscall 分发到 IO::write
-> 找当前进程
-> 翻译用户 buf
-> 判断 fd 类型
-> stdout 或普通文件或图形 fd
-> 完成写入
-> 返回用户态
```

所以 `write` 的关键不只是“写数据”，而是三件事：

- 找到当前进程。
- 根据 fd 找到目标对象。
- 把用户虚拟地址翻译成内核可访问地址。

## 7. open 的基本流程

`open(path, flags)` 的路径 `path` 同样是用户虚拟地址。

内核要做：

```text
读取用户传来的 path 指针
-> 逐字节翻译/读取直到 '\0'
-> 得到文件名字符串
-> 根据 flags 判断是否创建
-> 调用 FS.open
-> 得到 OSInode
-> 放入当前进程 fd_table
-> 返回 fd 下标
```

所以 open 的返回值不是文件本身，而是当前进程 fd 表中的下标。

## 8. close 的意义

`close(fd)` 的本质是从当前进程的 `fd_table` 中释放这个引用。

如果没有 close：

- 文件对象引用可能一直存在。
- 后续 fd 分配可能无法复用。
- 资源生命周期会混乱。

在 Rust 里，因为有 `Arc`，当最后一个引用消失时对象才真正释放。

## 9. easy-fs 是什么

easy-fs 是教学用的简单文件系统。它不是 ext4，也不是 FAT，而是为了教学把文件系统核心概念压缩成能理解的结构。

它包含：

- super block：文件系统整体信息。
- inode bitmap：哪些 inode 被占用。
- data bitmap：哪些数据块被占用。
- inode area：保存磁盘 inode。
- data area：保存文件内容和目录项。

从抽象上看：

```text
文件名
-> 目录项 DirEntry
-> inode 编号
-> DiskInode
-> direct/indirect 数据块
-> 块设备上的真实数据
```

## 10. 块设备和 fs.img 的关系

QEMU 运行 ch6 时会挂载一个磁盘镜像：

```text
fs.img
-> QEMU virtio-blk-device
-> 内核 VirtIO block driver
-> easy-fs
-> Inode/read/write
```

所以用户程序并不是直接改 Windows 上的文件。用户程序通过系统调用进入内核，内核通过 easy-fs 和块设备驱动修改 QEMU 看到的 `fs.img`。

可以理解为：

```text
用户程序写文件
-> 内核写 easy-fs
-> easy-fs 写块缓存
-> 块缓存同步到 virtio-blk
-> QEMU 写 fs.img
```

## 11. block cache 的作用

磁盘是按块读写的，文件系统不会每写一个字节都直接访问设备。

block cache 的作用是：

- 把磁盘块读到内存。
- 在内存中修改块内容。
- 合适时机同步回块设备。

这样可以减少频繁 I/O，也让文件系统代码更容易操作结构体。

## 12. inode 是什么

inode 是文件系统里描述一个文件的核心结构。

它不直接等于文件名。文件名存在目录项里，目录项指向 inode。

inode 里保存：

- 文件大小。
- 文件类型。
- 直接数据块指针。
- 一级间接块。
- 二级间接块。
- 本实验补充的硬链接计数 `nlink`。

文件名和 inode 的关系类似：

```text
"hello.txt" -> inode 12 -> 数据块列表 -> 文件内容
```

## 13. linkat 和 unlinkat 为什么难

`linkat` 和 `unlinkat` 操作的不是文件内容，而是目录项和 inode 引用关系。

`linkat(old, new)` 的语义：

```text
找到 old 对应 inode
在目录中增加 new 目录项
new 指向同一个 inode
inode.nlink += 1
```

`unlinkat(path)` 的语义：

```text
删除 path 对应目录项
inode.nlink -= 1
如果 nlink == 0
    回收文件数据块
    回收 inode
```

这就是为什么本次实现要修改 easy-fs 的 `layout.rs`、`efs.rs`、`vfs.rs`。

## 14. fstat 的意义

`fstat(fd, st)` 是让用户通过 fd 查询文件元信息。

它不是读取文件内容，而是读取文件属性：

- inode 编号。
- 文件类型。
- 硬链接数量。
- 设备号等字段。

流程是：

```text
用户传入 fd 和 st 指针
-> 内核找 fd_table[fd]
-> 找到 OSInode / Inode
-> inode.stat()
-> 把 Stat 写回用户地址 st
```

这里再次体现用户地址翻译：`st` 是用户虚拟地址，内核写之前必须翻译。

## 15. ch6 练习实现点

本次 ch6 练习主要补了：

- `linkat`
- `unlinkat`
- `fstat`
- `spawn`
- `mmap`
- `munmap`

其中 `spawn/mmap/munmap` 是继承 ch5/ch4 的能力，但要适配 ch6 的代码结构。

真正体现 ch6 文件系统主题的是：

- hard link。
- unlink 后资源回收。
- fstat 元信息查询。
- 文件 fd 的读写。

## 16. 组件化仓库和 Guide 的差异

Guide 里的代码树通常长这样：

```text
os/src/
├── fs
├── mm
├── syscall
├── task
├── trap
└── drivers
```

当前组件化仓库把很多能力拆到 crate 或集中在 `main.rs` 的 trait impl 里：

```text
tg-rcore-tutorial-ch6/src/main.rs
-> syscall trait impl
-> 内核启动
-> MMIO 映射
-> FS 初始化

tg-rcore-tutorial-ch6/src/fs.rs
-> 文件系统管理器
-> OSInode
-> BlockDevice

tg-rcore-tutorial-easy-fs/
-> easy-fs 真实文件系统结构

tg-rcore-tutorial-ch6/src/process.rs
-> Process 和 fd_table

tg-rcore-tutorial-ch6/src/processor.rs
-> 进程管理和调度
```

所以读代码时不要被文件名差异卡住，要按功能映射。

## 17. ch6 启动总流程

更细的启动流程可以拆成：

1. QEMU 启动 RISC-V virt 机器。
2. 加载内核 ELF。
3. 配置 virtio-blk 设备，并挂载 `fs.img`。
4. 内核进入 `rust_main`。
5. 初始化日志。
6. 输出内核段信息。
7. 建立内核地址空间。
8. 映射块设备 MMIO。
9. 本实验额外映射 GPU/keyboard MMIO。
10. 初始化堆分配器。
11. 初始化 trap/syscall 上下文。
12. 初始化文件系统 `FS`。
13. 通过 easy-fs 打开根目录。
14. 根据 `CHAPTER` 决定初始用户程序。
15. 默认运行 `ch6_breakout`。
16. 测试模式运行 ch6 usertests。
17. 从文件系统读取 ELF。
18. 创建初始进程。
19. 放入进程管理器。
20. 调度器取出进程。
21. 切换到用户态。
22. 用户程序发起 syscall。
23. trap 进入内核。
24. syscall trait 分发。
25. 文件系统或进程系统完成操作。
26. 返回用户态。
27. 所有进程结束后输出 `no task`。

## 18. 文件写入调用链

以 `filetest_simple` 写文件为例：

```text
user filetest_simple
-> user_lib::open
-> sys_open
-> ecall
-> kernel IO::open
-> FS.open
-> Inode::create/find
-> fd_table 分配 fd

user_lib::write(fd, buf)
-> sys_write
-> ecall
-> kernel IO::write
-> translate 用户 buf
-> fd_table[fd]
-> OSInode::write
-> Inode::write_at
-> DiskInode 数据块定位
-> block cache
-> virtio-blk
-> fs.img
```

读文件则是反向从块设备和 block cache 读回用户 buffer。

## 19. ch6-breakout 扩展目标

课程扩展要求是基于 ch6 实现用户态打砖块游戏，支持：

- 碰撞反弹。
- 计分。
- 保存进度。
- 恢复进度。

这个任务正好对应 ch6 文件系统：

- 图像输出需要设备抽象。
- 键盘输入需要输入设备抽象。
- 保存/恢复需要文件系统。

所以它不是单纯“画个小游戏”，而是把 ch6 文件能力用起来。

## 20. breakout 的用户态逻辑

用户态程序是：

```text
tg-rcore-tutorial-user/src/bin/ch6_breakout.rs
```

它维护一个 `Game`：

- paddle 位置。
- ball 位置和速度。
- bricks 数组。
- score。
- lives。
- level。
- saved 状态。

每一帧：

```text
读取键盘
-> 更新挡板
-> 更新小球
-> 判断墙壁/挡板/砖块碰撞
-> 必要时保存或恢复
-> 构造 BreakoutFrame
-> write(GRAPHICS_FD, frame)
-> sleep/yield
```

## 21. 保存和恢复为什么体现 ch6

按 `S` 保存时，用户程序：

```text
open("breakout.sav", CREATE | WRONLY)
-> write 保存结构体 SaveData
-> close
```

按 `R` 恢复时：

```text
open("breakout.sav", RDONLY)
-> read SaveData
-> 校验 magic
-> 恢复 paddle/ball/bricks/score/lives/level
-> close
```

这说明游戏状态不是只存在内存里，而是写入了文件系统。

## 22. 图形 fd 为什么设成 3

本实验中：

```text
fd 0 -> stdin
fd 1 -> stdout
fd 2 -> 保留/常见 stderr
fd 3 -> 特殊图形输出
```

用户程序对 `fd=3` 调用 `write`，内核不把它当普通文件，而是识别出：

```rust
if fd == crate::graphics::GRAPHICS_FD {
    submit_breakout_frame(...)
}
```

这就是“一切皆文件”的一个教学版扩展：图形设备也可以被包装成一个可写 fd。

## 23. 键盘输入如何接入

键盘输入走 `read(0, buf, count)`。

内核里如果发现 `fd == STDIN`：

```text
keyboard::take()
-> VirtIOInput 取 pending event
-> keycode 转 ASCII
-> 写入用户 buffer
```

如果没有键，返回 `-2`，用户程序就继续游戏循环，不会卡死等待输入。

## 24. 为什么 MMIO 范围要扩大

原本 ch6 只需要 virtio-blk：

```text
0x1000_1000
```

breakout 又加了：

```text
0x1000_2000 -> virtio-gpu
0x1000_3000 -> virtio-keyboard
```

所以内核 MMIO 映射范围从一个设备扩展为三个设备：

```rust
pub const MMIO: &[(usize, usize)] = &[(0x1000_1000, 0x00_3000)];
```

没有这个映射，内核访问 GPU/keyboard 的 MMIO 地址会出错。

## 25. ch6 测试结果理解

本地验证过：

```text
CHAPTER=6 cargo run --features exercise
-> ch6 Usertests passed!

CHAPTER=-6 cargo run
-> Basic usertests passed!

cargo build
-> 编译通过
```

输出中会出现一些 `StorePageFault` 或文件测试中的 panic 信息，但这些是测试用例用来检查异常处理和文件语义的一部分。最终 checker 认可通过才是关键。

## 26. 这一章我最应该记住什么

ch6 的核心不是“会写文件 API”，而是理解这条链：

```text
用户进程
-> syscall
-> 当前进程 fd_table
-> OSInode / Console / 特殊设备 fd
-> Inode
-> easy-fs
-> block cache
-> virtio-blk
-> fs.img
```

只要这条链通了，就能理解为什么文件系统是 OS 的核心抽象之一。
## GTK QEMU 图形 Demo 补充记录

本章在原有文件系统、块设备和 fd 抽象之外，额外做了一个 `ch6-breakout` 图形展示路径。为了避免 `cargo run` 时只看到终端字符画，我把默认运行方式调整为 QEMU GTK 窗口，并通过 VirtIO-GPU 直接把固定脚本帧渲染到 framebuffer。

演示图如下：

![ch6-breakout-demo](../assets/ch6-breakout-demo.gif)

这条 demo 路径和基础测试路径是分开的：`cargo run` 默认展示 Breakout 图形 demo；`test.sh base` 和 `test.sh exercise` 会覆盖 `CHAPTER` 和 runner，继续走原教程的 headless 测试流程。因此它不会破坏 ch6 的文件系统测试。

这次调试中最重要的结论是：图形展示不是普通 `println!`，而是内核通过 MMIO 发现 VirtIO-GPU，初始化 framebuffer，再把每一帧像素写入显存区域并 flush 给 QEMU 显示。最开始我误以为是 GPU 初始化失败，后来通过串口日志确认已经到了 `virtio-gpu ready`，真正的瓶颈是逐像素绘制太慢，于是改为按 4 字节 BGRA 批量写矩形，最终日志能稳定出现：

```text
[ch6-breakout] virtio-gpu ready
[ch6-breakout] first frame visible
[ch6-breakout] Test ch6 breakout OK!
```
