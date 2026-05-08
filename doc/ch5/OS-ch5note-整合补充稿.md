# rCore ch5 进程管理整合补充稿

## 1. 这一章到底在解决什么问题

ch5 的主题是进程。前面几章已经一步步把“程序能跑起来”这件事拆开了：

- ch1 解决裸机启动、输出、QEMU 运行环境。
- ch2 解决批处理系统，一个应用结束后再运行下一个应用。
- ch3 解决多道程序和分时调度，多个应用能轮流占用 CPU。
- ch4 解决地址空间，每个应用看到自己的虚拟地址，内核通过页表把它映射到真实物理页。

ch5 在 ch4 的基础上继续向真实操作系统靠近。它不再只是“内核启动时提前加载一批程序，然后按队列执行”，而是引入了进程生命周期：进程可以创建子进程、替换自己的程序内容、等待子进程结束、回收子进程资源。

一句话概括：ch5 把“一个正在运行的程序”封装成可被内核管理的对象，也就是进程。

## 2. 从应用到进程：为什么需要再套一层抽象

在 ch4 中，一个用户程序已经拥有独立地址空间和执行上下文，但它依然更像是“内核提前准备好的执行单元”。ch5 里的进程更接近 Linux 中的进程概念，它不仅包含代码和数据，还包含：

- 进程编号 PID。
- 当前寄存器上下文。
- 独立地址空间。
- 堆边界。
- 父进程。
- 子进程列表。
- 退出码。
- 当前运行状态。
- 调度用的优先级和 stride 信息。

这和我原先熟悉的 Spring Boot 里的“一个服务对象被框架管理”有一点类比关系：程序本身只是代码，进程是内核把代码、内存、上下文、父子关系、状态都包装起来之后形成的运行实体。

## 3. shell 为什么也是一个进程

一开始我容易把 shell 理解成“操作系统本身的一部分”。这一章要纠正这个直觉：shell 也是用户态程序。

它和其他程序的区别只是它长期运行，并负责：

- 读取用户输入的命令。
- 根据命令 fork 出子进程。
- 让子进程 exec 成目标程序。
- 等待子进程退出。
- 回收子进程资源。
- 回到命令行继续等待下一条命令。

所以 shell 不是一个神秘的内核功能，而是一个普通用户程序。真正的权限切换、地址空间切换、进程调度仍然由内核完成。

## 4. fork/exec/wait/exit 的整体语义

ch5 最核心的四个系统调用是 `fork`、`exec`、`wait`、`exit`。

`fork` 的意思是复制当前进程，创建一个子进程。子进程继承父进程的地址空间内容、上下文和一部分进程属性。fork 后，父进程和子进程都会从 fork 返回处继续执行，但返回值不同，父进程拿到子进程 PID，子进程拿到 0。

`exec` 的意思是替换当前进程的程序内容。它不会新建 PID，而是把当前进程的地址空间、用户上下文、入口地址替换成另一个 ELF 程序。

`wait` 的意思是父进程等待某个子进程退出。如果子进程已经变成 Zombie，就回收它并拿到退出码；如果还没有退出，就父进程先让出 CPU。

`exit` 的意思是当前进程结束。它不是简单“函数返回”，而是通知内核把进程标记为 Zombie，并保存退出码，等待父进程回收。

## 5. 为什么 fork 后通常还要 exec

一开始我觉得 fork + exec 很奇怪：为什么不能直接创建一个新进程来运行目标程序？

原因是 fork 和 exec 分别表达两个动作：

- fork：创建一个和自己几乎一样的新进程。
- exec：把当前进程替换成另一个程序。

二者组合起来就能实现 shell 常见行为：shell 先 fork 一个子进程，子进程再 exec 成目标程序，而 shell 父进程自己继续存在，可以 wait 子进程。

如果 shell 自己直接 exec 成目标程序，那么 shell 就被替换掉了，命令行也就没了。正因为 shell 要继续工作，所以它必须 fork 子进程，让子进程去 exec。

## 6. ch5 exercise 为什么要求实现 spawn

实验题让我们实现 `spawn(path, count)`，它相当于把 fork + exec 的常见组合做成一个一步到位的系统调用。

spawn 的语义是：

```text
当前进程请求内核：
    请创建一个新的子进程
    让它直接运行 path 指向的目标程序
成功返回子进程 PID
失败返回 -1
```

和 fork 不同，spawn 不需要复制父进程完整地址空间。它可以直接根据目标 ELF 创建新进程，这通常更轻量。

本实验中 spawn 的实现主线是：

```text
用户态传入 path 虚拟地址
-> 内核根据当前进程页表翻译 path
-> 从 APPS 表中查找目标 ELF
-> Process::from_elf 创建新进程
-> ProcManager::add 挂到当前进程子进程列表
-> 返回新 PID
```

## 7. ch5 为什么还要迁移 mmap/munmap

ch4 已经实现过 `mmap` 和 `munmap`，但 ch5 的进程结构变了。

ch4 中进程可能保存在一个简单的 `PROCESSES` 数组/列表里。ch5 中进程由 `ProcManager` 管理，当前进程要通过 `PROCESSOR.get_mut().current()` 获得。因此同样的内存映射逻辑需要迁移到新的进程管理结构。

迁移不是简单复制代码，而是要改“从哪里找到当前进程”：

```text
ch4:
    PROCESSES[caller.entity].address_space

ch5:
    PROCESSOR.get_mut().current().unwrap().address_space
```

这体现了 ch5 的重点：内核不再只管理一个简单进程数组，而是有一个进程管理器负责当前进程、就绪队列、父子关系和调度。

## 8. stride 调度算法的直觉

ch3 的调度类似轮流来：大家排队，每个人运行一会儿。

ch5 的 stride 调度引入优先级。优先级越高，一个进程应该获得越多 CPU 时间。

它的做法很像“谁欠得少就先运行”：

- 每个进程有一个 `stride`，表示它已经累计获得的运行份额。
- 每个进程有一个 `priority`。
- 每次选择 `stride` 最小的进程运行。
- 运行后，该进程的 stride 增加 `pass = BIG_STRIDE / priority`。

priority 越大，pass 越小，stride 增长越慢，于是更容易再次成为 stride 最小者，也就更频繁被调度。

## 9. stride 为什么用大整数

`BIG_STRIDE / priority` 需要尽量减少整数除法误差。`BIG_STRIDE` 太小会导致不同 priority 算出来的 pass 过于接近，调度比例不准。

我们使用：

```rust
const BIG_STRIDE: u128 = 1 << 60;
```

并用 `u128` 保存 stride，避免长时间累加后溢出太快。

## 10. set_priority 的边界

实验要求 `priority >= 2`，否则返回 -1。

原因是 priority 太小会破坏 pass 计算和调度语义。我们实现时保留：

```text
if prio < 2:
    return -1
else:
    current.priority = prio
    return prio
```

进程初始 priority 是 16，初始 stride 是 0。

## 11. ch5 的组件化结构

本仓库和原始 Guide 的结构有差异。Guide 里通常有 `task`、`mm`、`trap`、`syscall` 等目录，而组件化版本把很多能力拆到了独立 crate 或更紧凑的模块中。

ch5 主要相关模块如下：

```text
tg-rcore-tutorial-ch5/
├── build.rs
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── process.rs
│   ├── processor.rs
│   ├── graphics.rs
│   └── keyboard.rs
└── test.sh

tg-rcore-tutorial-user/
└── src/bin/
    ├── ch5_usertest.rs
    ├── ch5_spawn0.rs
    ├── ch5_spawn1.rs
    ├── ch5_stride*.rs
    └── ch5_pingpong.rs
```

其中：

- `build.rs`：决定打包哪些用户程序进内核。
- `main.rs`：内核启动、系统调用实现、内核地址空间、初始进程选择。
- `process.rs`：进程结构和 ELF 加载、fork、exec、sbrk 等逻辑。
- `processor.rs`：进程管理器、当前进程、就绪队列、stride 调度。
- `graphics.rs`：pingpong 扩展用 VirtIO-GPU 输出。
- `keyboard.rs`：pingpong 扩展用 VirtIO-keyboard 输入。

## 12. Guide 代码树和组件化代码的对应关系

Guide 中的概念在组件化仓库中的对应关系大致如下：

```text
Guide: TaskControlBlock / ProcessControlBlock
-> ch5/src/process.rs::Process

Guide: TaskManager / Processor
-> ch5/src/processor.rs::ProcManager + PROCESSOR

Guide: MemorySet / AddressSpace
-> tg-rcore-tutorial-kernel-vm::AddressSpace

Guide: frame_allocator
-> tg-rcore-tutorial-kernel-alloc

Guide: syscall/process.rs
-> ch5/src/main.rs::impls::Process

Guide: syscall/mm.rs
-> ch5/src/main.rs::impls::Memory

Guide: syscall/fs.rs
-> ch5/src/main.rs::impls::IO

Guide: loader/app manager
-> build.rs + APPS Lazy map
```

组件化版本把多个 Guide 文件压缩到了 `main.rs::impls` 和外部 crate 中，所以读代码时不能只按 Guide 的目录树找，而要按功能找。

## 13. ch5 启动流程总览

ch5 启动时发生的事情可以概括为：

1. Cargo 调用 `build.rs`。
2. `build.rs` 读取 `cases.toml`。
3. 根据 feature 和 `CHAPTER` 选择 case。
4. 默认选择 `ch5_pingpong`。
5. exercise 选择 `ch5_exercise`。
6. base 测试选择 `ch5`。
7. 编译对应用户态 ELF。
8. 生成 `app.asm`。
9. 内核通过 `global_asm!(include_str!(env!("APP_ASM")))` 把用户程序嵌入。
10. QEMU 启动内核。
11. `rust_main` 清 BSS。
12. 初始化 console 和 log。
13. 初始化内核堆。
14. 分配 portal 页面。
15. 建立内核地址空间。
16. 初始化 syscall traits。
17. 根据 `CHAPTER` 选择初始进程名。
18. 从 `APPS` 找到初始 ELF。
19. `Process::from_elf` 创建进程。
20. `map_portal` 将传送门映射复制到用户地址空间。
21. `ProcManager::add` 加入进程管理器。
22. 进入 `schedule` 调度循环。
23. 通过 portal 切换到用户地址空间。
24. 用户进程执行。
25. 用户进程 ecall 返回内核。
26. 内核处理 syscall 或异常。
27. 进程 yield/exit/wait/fork/exec/spawn 改变调度状态。
28. ProcManager 选择下一个进程。
29. 所有进程结束时输出 `no task`。

## 14. Process 结构体的意义

`Process` 是 ch5 的核心抽象。它不是单纯的“代码”，而是运行时状态集合。

典型字段包括：

```text
pid              进程编号
context          用户执行上下文，含寄存器和 satp 等信息
address_space    当前进程页表和映射关系
heap_bottom      用户堆起点
program_brk      当前堆边界
parent           父进程 PID
children         子进程 PID 列表
exit_code        退出码
priority         stride 调度优先级
stride           stride 调度累计值
```

可以把它理解为“内核给一个用户程序建立的档案袋”。程序每次被调度、被打断、被恢复，都要依靠这个档案袋里的信息。

## 15. ProcManager 的意义

`ProcManager` 负责管理所有进程和调度。

它主要保存：

```text
tasks       PID -> Process 的映射
ready_queue 可运行 PID 队列
current     当前正在运行的 PID
```

ch5 的 `fetch()` 不再简单弹出队首，而是扫描 ready queue，找到 stride 最小的进程运行。

这就是从“队列轮转”向“按优先级比例分配 CPU”的过渡。

## 16. fork 的流程

fork 的核心流程是：

1. 用户程序调用 `fork()`。
2. 用户态封装触发 ecall。
3. 内核进入 syscall 分发。
4. `impls::Process::fork` 被调用。
5. 取出当前进程。
6. 调用 `Process::fork` 复制地址空间。
7. 新进程获得新 PID。
8. 新进程 parent 指向当前 PID。
9. 当前进程 children 增加子 PID。
10. 子进程返回值被设置为 0。
11. 父进程获得子 PID。
12. 子进程加入 ready queue。

fork 的难点在于：同一段代码后续会在两个进程里继续执行，但返回值不同。

## 17. exec 的流程

exec 的核心流程是：

1. 用户程序传入目标程序名。
2. 内核翻译用户态字符串地址。
3. 从 `APPS` 表中查找目标 ELF。
4. 解析 ELF。
5. 构建新的地址空间。
6. 重新映射代码段、数据段、用户栈。
7. 重建用户上下文。
8. 保留当前 PID 和进程关系。
9. 替换当前进程的 `address_space` 和 `context`。
10. 返回用户态后执行新程序入口。

exec 不是新建进程，而是“换身体不换身份证”。

## 18. wait 的流程

wait 的核心流程是：

1. 父进程调用 wait。
2. 内核检查当前进程 children。
3. 如果没有目标子进程，返回 -1。
4. 如果目标子进程还没退出，父进程让出 CPU。
5. 如果子进程已经 Zombie，读取退出码。
6. 将退出码写到用户传入的地址。
7. 从任务表中删除子进程。
8. 从 children 列表移除子 PID。
9. 返回被回收的子 PID。

wait 是资源回收机制。如果没有 wait，退出的子进程会一直占着内核记录。

## 19. exit 的流程

exit 的核心流程是：

1. 用户进程调用 exit。
2. 内核记录 exit code。
3. 当前进程变为 Zombie。
4. 子进程可能被托管给 init 进程。
5. 当前进程不再进入 ready queue。
6. 父进程 wait 时回收它。

exit 不是简单停止 CPU，而是进入“等待父进程收尸”的状态。

## 20. spawn 的实现思路

spawn 是我们本章 exercise 的重点。

实现中要注意用户传进来的 path 是用户虚拟地址，不能直接当内核地址读。必须：

```text
VAddr(path)
-> current.address_space.translate()
-> 得到内核可访问指针
-> from_raw_parts(ptr, count)
-> from_utf8_unchecked 得到程序名
```

然后：

```text
APPS.get(name)
-> ElfFile::new()
-> Process::from_elf()
-> ProcManager::add(parent_pid, child_proc)
-> return child_pid
```

这里体现了 ch4 地址空间知识在 ch5 中继续使用：系统调用的参数如果是指针，内核必须做地址翻译和权限检查。

## 21. mmap/munmap 的迁移流程

`mmap`：

1. 检查起始地址是否页对齐。
2. 检查长度是否为 0。
3. 检查 prot 权限是否合法。
4. 计算虚拟页区间。
5. 检查这些页是否已经映射。
6. 如果已有映射，返回 -1。
7. 调用 `address_space.map` 建立映射。
8. 返回 0。

`munmap`：

1. 检查起始地址是否页对齐。
2. 计算虚拟页区间。
3. 检查所有页是否已经映射。
4. 如果有未映射页，返回 -1。
5. 调用 `address_space.unmap` 取消映射。
6. 返回 0。

关键不是算法变复杂，而是当前进程的定位方式变了。

## 22. pingpong 游戏扩展

本章扩展任务要求基于 ch5 做双进程协作乒乓游戏。当前实现采用用户态游戏逻辑 + 内核图形/键盘服务的方式：

```text
user/src/bin/ch5_pingpong.rs
-> 维护球、左右挡板、比分、速度
-> 通过 read(stdin) 读取按键
-> 通过 write(fd=3) 提交画面帧

ch5/src/main.rs
-> read(stdin) 从 VirtIO-keyboard/UART 获取输入
-> write(fd=3) 调用 graphics::submit_pingpong_frame

ch5/src/graphics.rs
-> 初始化 VirtIO-GPU
-> 根据 PingpongFrame 绘制球场、挡板、球、比分

ch5/src/keyboard.rs
-> 初始化 VirtIO-keyboard
-> 把键码转换为 w/s/i/k/q
```

默认 `cargo run` 启动 pingpong，`CHAPTER=5 cargo run --features exercise` 启动练习测试。

## 23. 图形化调试中遇到的问题

本章 pingpong 一开始遇到了两个典型工程问题。

第一个问题是 `cargo clone` 不存在。原因是 build.rs 找不到本地 `tg-rcore-tutorial-user`，就尝试用 `cargo clone` 拉 crates.io 包。本地没有安装 `cargo-clone`，所以失败。解决方式是在 `.cargo/config.toml` 中固定：

```toml
TG_USER_DIR = "C:\\Users\\FLY\\Desktop\\OS\\...\\tg-rcore-tutorial-user"
```

第二个问题是 VirtIO-GPU 初始化失败。日志显示设备能识别，但 `setup_framebuffer()` 失败。原因是 framebuffer 需要的 DMA 内存不足。800x480x4 字节约 1.46MB，而 128 页 DMA 只有 512KB。解决方式是降低分辨率到 640x360，并把 GPU DMA 调整为 256 页。

## 24. ch5 的测试结果

已验证：

```text
CHAPTER=5 cargo run --features exercise
-> ch5 Usertests passed!

CHAPTER=-5 cargo run
-> Basic usertests passed!

cargo build
-> 默认 pingpong 构建成功

cargo run
-> 日志出现:
   [ch5-pingpong] virtio-gpu ready
   [ch5-pingpong] virtio-keyboard ready
```

## 25. 本章最重要的理解

ch5 的本质不是“多了几个系统调用”，而是操作系统开始管理程序的生命周期。

ch3 主要是任务之间轮流跑。

ch4 主要是每个任务有独立地址空间。

ch5 进一步把任务组织成父子关系，并允许程序动态创建、替换、等待、退出。

也就是说，ch5 让系统从“内核安排好的多个程序”变成“用户程序可以通过系统调用动态组织其他程序”。

## 26. 我需要继续巩固的点

- fork 后父子进程为什么都从同一位置继续执行。
- exec 为什么保留 PID 但替换地址空间。
- wait 为什么必须和 Zombie 状态配合。
- spawn 为什么不需要复制父进程地址空间。
- stride 调度为什么 priority 越大 pass 越小。
- 用户指针为什么必须经过页表翻译。
- 图形设备为什么需要 DMA 和 framebuffer。

## 27. 本章一句话复述

ch5 是把 ch4 中“能运行的独立地址空间”进一步封装成“有父子关系、生命周期和调度属性的进程”，并通过 fork/exec/wait/exit/spawn/set_priority 这些系统调用，让用户程序能够主动组织和管理其他程序。
