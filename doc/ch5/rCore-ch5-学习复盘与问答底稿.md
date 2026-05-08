# rCore ch5 学习复盘与问答底稿

## 复盘目标

这一份文档不是单纯记录“我做了什么”，而是把学习过程中容易混淆的问题整理成问答。目标是以后复习时能迅速回答：

- ch5 为什么引入进程？
- 进程和程序有什么区别？
- fork/exec/wait/exit 到底分别干什么？
- spawn 为什么和 fork 不一样？
- stride 调度为什么能体现优先级？
- ch5 代码树和 Guide 中的传统结构如何对应？
- pingpong 游戏扩展如何体现系统调用和设备抽象？

## Q1：ch5 和前面几章的关系是什么？

我的初始理解：

> 前面是把程序跑起来，ch5 好像就是又套了一层进程壳子。

修正后的理解：

这个说法方向是对的，但要更精确。ch5 不是简单“套壳”，而是把一个正在运行的程序抽象成内核可管理的进程对象。

前几章的递进关系是：

```text
ch1: 裸机能启动，能输出。
ch2: 多个程序能批处理。
ch3: 多个程序能分时轮流运行。
ch4: 每个程序有独立虚拟地址空间。
ch5: 程序运行实体有 PID、父子关系、生命周期和调度属性。
```

所以 ch5 的重点是生命周期管理。

## Q2：进程和程序有什么区别？

我的初始理解：

> 程序就是用户级执行的东西，进程像它外面的管理结构。

修正后的答案：

程序是静态的代码和数据，通常是 ELF 文件。进程是程序运行起来以后，内核为它建立的运行实体。

进程包括：

- 程序代码和数据。
- 当前寄存器上下文。
- 地址空间。
- PID。
- 父子关系。
- 堆边界。
- 调度信息。
- 退出状态。

可以类比为：

```text
程序 = 菜谱
进程 = 正在按照菜谱做菜的人 + 厨房 + 工具 + 当前做到第几步
```

## Q3：为什么 shell 也是用户进程？

我的初始理解：

> shell 像终端监督者，父进程就是终端过程。

修正后的答案：

shell 是一个普通用户程序，只是它长期运行并负责启动其他程序。

它的循环大致是：

```text
读取命令
fork 子进程
子进程 exec 目标程序
父进程 wait 子进程
回到命令行
```

它不是内核。它没有直接硬件权限，也要通过 syscall 请求内核。

## Q4：为什么 shell 不直接 exec？

我的初始理解：

> shell 自己 exec 后功能就执行不了了。

修正后的答案：

完全正确。exec 会替换当前进程的地址空间和上下文。如果 shell 自己 exec 成目标程序，那么 shell 进程本身就变成了目标程序。目标程序退出后，原 shell 不存在了，命令行无法继续。

因此 shell 必须：

```text
父进程 shell 保留
子进程 exec 成目标程序
父进程 wait 子进程结束
```

## Q5：fork 为什么会让父子进程从同一位置继续？

我的初始疑问：

> fork 之后为什么父子都像从同一行返回？

修正后的答案：

fork 复制的是当前进程的地址空间和上下文。上下文里包含“当前执行到哪里”和寄存器状态。所以子进程被调度时，也会从 fork 返回后继续执行。

区别由返回值区分：

```text
父进程 fork 返回 child_pid
子进程 fork 返回 0
失败返回 -1
```

内核会专门修改子进程上下文中的返回寄存器，让子进程看到 0。

## Q6：exec 为什么不新建 PID？

我的初始理解：

> exec 是替换当前进程。

修正后的答案：

是的。exec 的含义是替换当前进程的用户程序内容，而不是创建新进程。

它会替换：

- 地址空间。
- 用户代码。
- 用户数据。
- 用户栈。
- 入口地址。
- 用户上下文。

它保留：

- PID。
- 父子关系。
- 当前进程在进程表中的身份。

所以 exec 是“换身体，不换身份证”。

## Q7：wait 为什么必要？

我的初始理解：

> 父进程等子进程弄完，再回到命令行。

修正后的答案：

wait 不只是等待，还负责回收资源。

子进程 exit 后会变成 Zombie。Zombie 不再运行，但内核还保留它的退出码和进程记录，等待父进程读取。

wait 做的事情：

1. 找到子进程。
2. 判断它是否已经退出。
3. 如果未退出，父进程让出 CPU。
4. 如果已退出，读取 exit_code。
5. 写回用户传入的地址。
6. 删除子进程记录。
7. 返回子进程 PID。

## Q8：exit 和 main return 有什么关系？

修正后的答案：

用户程序 `main` 返回后，用户库最终也会通过 exit 告诉内核：当前进程结束。

exit 的关键不是“返回一个数”，而是让内核改变进程状态：

```text
Running -> Zombie
保存 exit_code
从 ready_queue 移除
等待父进程 wait
```

## Q9：spawn 和 fork/exec 的区别是什么？

我的初始理解：

> spawn 是直接请求内核创建一个子进程。

修正后的答案：

对。spawn 可以理解为“一步创建并运行目标程序”。

区别：

```text
fork:
    复制当前进程

exec:
    替换当前进程程序内容

fork + exec:
    复制出子进程，再让子进程替换成目标程序

spawn:
    直接根据目标 ELF 创建子进程
```

因此 spawn 不必复制父进程完整地址空间，效率更高。

## Q10：spawn 为什么必须翻译用户指针？

我的初始理解：

> 用户传进来的 path 是地址，内核要找真实物理地址。

修正后的答案：

准确。ch4 引入地址空间后，用户程序传给内核的指针都是用户虚拟地址。内核不能直接解引用这个地址，因为内核当前运行在内核地址空间中。

spawn 中 path 的处理必须是：

```text
path 用户虚拟地址
-> 当前进程 address_space.translate
-> 得到内核可访问的物理映射指针
-> 读取 count 字节
-> 得到程序名字符串
```

这体现了 ch4 的知识在 ch5 syscall 中继续发挥作用。

## Q11：mmap/munmap 为什么要迁移？

我的初始疑问：

> ch4 不是已经写过了吗，为什么 ch5 还要迁移？

修正后的答案：

因为进程管理结构变了。

ch4 的当前进程可能通过简单数组下标找到，ch5 要通过 `PROCESSOR` 和 `ProcManager` 找当前进程。

因此真正变化的是访问路径：

```text
ch4:
    PROCESSES[caller.entity].address_space

ch5:
    PROCESSOR.get_mut().current().unwrap().address_space
```

mmap/munmap 的页对齐、权限检查、映射检查逻辑基本继承 ch4。

## Q12：Process 结构体应该怎么理解？

修正后的答案：

`Process` 是单个进程的档案袋。

它保存：

```text
pid
context
address_space
heap_bottom
program_brk
parent
children
exit_code
priority
stride
```

如果只看用户程序代码，是看不到这些内容的。它们是内核为了管理程序运行而额外维护的状态。

## Q13：ProcManager 和 Process 有什么区别？

我的初始理解：

> Process 是一个程序，ProcManager 管理这些状态和档案。

修正后的答案：

这个理解很好，可以再精确一点：

- `Process`：一个进程的状态。
- `ProcManager`：所有进程的集合、当前进程、ready queue、调度逻辑。

关系类似：

```text
Process = 一个学生档案
ProcManager = 教务系统
```

## Q14：ch5 的调度和 ch3 的调度有什么区别？

修正后的答案：

ch3 更像轮转调度：任务一个接一个被选中。

ch5 引入 stride 调度：每个进程有优先级，调度器根据 stride 最小原则选择进程。

ch5 调度不仅要决定“谁运行”，还要配合进程生命周期：

- fork 新增 ready 进程。
- spawn 新增 ready 进程。
- wait 可能挂起父进程。
- exit 让进程变 Zombie。
- set_priority 改变后续调度比例。

## Q15：stride 算法为什么 priority 越大越容易运行？

我的初始疑问：

> priority 大为什么不是加得更多？

修正后的答案：

stride 算法每次选择 stride 最小者运行。运行后：

```text
stride += BIG_STRIDE / priority
```

priority 越大，`BIG_STRIDE / priority` 越小。该进程运行后 stride 增长较慢，所以下次更容易保持较小 stride，再次被选中。

所以 priority 越大，pass 越小，运行机会越多。

## Q16：set_priority 为什么要求 prio >= 2？

修正后的答案：

实验约定 priority 必须 >= 2。这样可以避免非法优先级破坏调度逻辑。

实现时：

```text
prio < 2:
    return -1
prio >= 2:
    current.priority = prio
    return prio
```

## Q17：为什么 ch5 exercise 要前向兼容 ch4？

修正后的答案：

因为操作系统是逐层构建的。ch5 新增进程管理后，不能把 ch4 的地址空间和 mmap 功能弄坏。

所以 ch5 exercise 测试中包含：

- ch3 sleep。
- ch4 mmap/munmap。
- ch5 spawn。
- ch5 set_priority。
- ch5 stride。

这说明新功能不能破坏旧功能。

## Q18：为什么默认 cargo run 要和测试入口分开？

修正后的答案：

因为课程扩展任务要求默认运行游戏，但基础测试要求运行 usertest。

如果默认永远启动 `initproc` 或 shell，exercise 可能卡住。

如果默认永远启动游戏，CI 测试又不方便。

所以我们做了入口区分：

```text
CHAPTER=5 + --features exercise -> ch5_usertest
CHAPTER=-5                     -> ch5b_usertest
默认 cargo run                 -> ch5_pingpong
```

## Q19：为什么一开始 cargo run 会报 cargo clone？

问题现象：

```text
error: no such command: clone
failed to clone tg-rcore-tutorial-user@0.4.10
```

修正后的理解：

这不是内核错误，也不是 QEMU 错误，而是构建脚本没找到本地用户态目录，于是尝试调用 `cargo clone` 下载包。本机没装 `cargo-clone`，因此失败。

解决方式是在 `.cargo/config.toml` 中设置：

```toml
TG_USER_DIR = "C:\\Users\\FLY\\Desktop\\OS\\...\\tg-rcore-tutorial-user"
```

## Q20：为什么 VirtIO-GPU 会初始化失败？

问题现象：

```text
[ch5-pingpong] failed to initialize virtio-gpu
```

日志又显示：

```text
Device features ...
Config ...
```

修正后的理解：

这说明设备本身能识别，MMIO 映射也基本没错。失败发生在 framebuffer 建立阶段。

原因是 DMA 池不够：

```text
800 * 480 * 4 = 1,536,000 bytes
128 pages * 4096 = 524,288 bytes
```

解决方式：

```text
分辨率降到 640x360
framebuffer ≈ 921,600 bytes
GPU DMA 设置为 256 pages ≈ 1MB
```

## Q21：这是“爆栈”吗？

我的提问：

> 爆栈了吧？

修正后的答案：

不是函数调用栈爆了，而是 framebuffer/DMA 静态内存池不够。它更像“显卡驱动要一块连续内存作为画布，但画布太大，池子太小”。

不过它和内存布局有关。如果 DMA 池开太大，又会挤压内核可用堆，导致 spawn 测试中进程创建失败。所以需要折中。

## Q22：为什么 pingpong 要通过 fd=3 输出图形？

修正后的答案：

用户程序不应该直接碰硬件。它应该通过系统调用请求内核服务。

我们把 fd=3 约定为图形设备：

```text
用户态 write(fd=3, PingpongFrame)
-> 内核 IO::write
-> graphics::submit_pingpong_frame
-> VirtIO-GPU framebuffer
```

这类似 Linux 中“一切皆文件”的思想：不同 fd 可以代表 stdout、文件、管道，也可以代表我们教学实验中的图形设备。

## Q23：为什么 read(stdin) 可以读键盘？

修正后的答案：

用户态调用：

```text
try_getchar -> read(STDIN)
```

内核中：

```text
IO::read
-> input::take
-> keyboard::take
-> VirtIOInput::pop_pending_event
-> keycode_to_ascii
```

所以用户程序不需要知道 VirtIO-keyboard 的细节，只知道从 stdin 读字符。

## Q24：pingpong 是否真的体现了“双进程协作”？

当前实现主要是用户态单进程游戏循环，具备双人控制、碰撞、计分、速度变化和图形输出。它已经完成“用户态乒乓游戏 + 内核图形/键盘支持”的主线。

如果后续要更贴合“双进程协作”，可以继续扩展为：

```text
父进程负责画面和球
子进程负责输入或一侧挡板
父子通过 pipe 或共享协议通信
```

但在 ch5 的当前能力下，pipe 还在 ch7 才系统讲，因此本阶段先用单进程用户态实现游戏逻辑是更稳的选择。文档中可以说明这是“基于 ch5 进程系统的用户态交互游戏”，后续可在 ch7 用 pipe 改造成真正双进程协作版本。

## Q25：ch5 最重要的调用链是什么？

修正后的答案：

最能串起全章的是 spawn 流程：

```text
用户进程调用 spawn
-> ecall 进入内核
-> 内核翻译用户 path 指针
-> APPS 查找目标 ELF
-> Process::from_elf 创建新进程
-> ProcManager::add 建立父子关系
-> 子进程进入 ready_queue
-> stride 调度器选中子进程
-> 子进程运行 exit
-> 父进程 wait 回收
```

这条链同时包含：

- 用户态 syscall。
- 地址空间翻译。
- ELF 加载。
- 进程创建。
- 父子关系。
- 调度。
- 退出和回收。

## Q26：我现在能怎么讲 ch5？

可以这样讲：

```text
ch5 不是简单增加几个 syscall，而是让内核具备进程生命周期管理能力。进程不只是代码，而是 PID、地址空间、上下文、父子关系和调度信息的组合。fork 创建相似子进程，exec 替换当前程序，wait 回收子进程，exit 结束当前进程，spawn 则直接从目标 ELF 创建新进程。stride 调度让进程按优先级比例获得 CPU。pingpong 扩展则展示了用户程序如何通过 read/write syscall 使用键盘和图形设备。
```

## Q27：我还需要继续补强什么？

后续复习时重点看：

1. fork 的返回值分流。
2. exec 的地址空间替换。
3. Zombie 与 wait 的关系。
4. spawn 中用户指针翻译。
5. ProcManager 的 ready_queue 和 current。
6. stride/pass/priority 的数学关系。
7. fd=3 图形输出的系统调用路径。
8. DMA framebuffer 大小和内核内存布局的关系。

## Q28：本章自测题

### 题 1：为什么 shell 要 fork 子进程？

答：因为 exec 会替换当前进程。如果 shell 自己 exec，shell 就消失了。fork 子进程后，子进程 exec，父进程 shell 保留并 wait。

### 题 2：fork 和 spawn 的最大区别是什么？

答：fork 复制当前进程，spawn 直接从目标 ELF 创建新进程。

### 题 3：exec 后 PID 会变吗？

答：不会。exec 替换当前进程地址空间和上下文，但保留 PID 和父子关系。

### 题 4：为什么 wait 要处理 Zombie？

答：子进程 exit 后不立即删除，内核保留退出码等待父进程读取。wait 读取退出码并回收资源。

### 题 5：priority 越大为什么运行越多？

答：因为 pass = BIG_STRIDE / priority，priority 越大 pass 越小，stride 增长越慢，更容易被再次选中。

### 题 6：为什么用户 path 不能直接读？

答：path 是用户虚拟地址，内核当前不在该用户地址空间中，必须通过当前进程页表 translate 后才能安全访问。

### 题 7：VirtIO-GPU 初始化失败时怎么定位？

答：如果 features/config 能读到，说明设备和 MMIO 基本正确；如果 setup_framebuffer 失败，要检查 framebuffer 大小和 DMA 池大小是否匹配。

### 题 8：pingpong 中用户态和内核态分别做什么？

答：用户态做游戏逻辑，内核态提供键盘输入和 framebuffer 图形输出服务。

## Q29：和 AI 协作的记录摘要

本章协作过程可以分为：

1. 先读 Guide，理解进程概念。
2. 用通俗语言确认 shell、父进程、子进程、生命周期。
3. 分析 exercise.md，确定要实现 mmap/munmap、spawn、stride、set_priority。
4. 在 `process.rs` 增加 priority 和 stride。
5. 在 `processor.rs` 实现 stride fetch。
6. 在 `main.rs::impls::Process` 实现 spawn。
7. 在 `main.rs::impls::Memory` 迁移 mmap/munmap。
8. 在 `main.rs::impls::Scheduling` 实现 set_priority。
9. 发现 exercise 进入 shell，改初始进程选择。
10. 跑通 `ch5 Usertests passed!`。
11. 实现 pingpong 用户态程序。
12. 添加内核图形和键盘模块。
13. 解决 `cargo clone` 本地配置问题。
14. 解决 VirtIO-GPU framebuffer DMA 不足问题。
15. 推送 GitHub。
16. 补写四份 Markdown 笔记。

## Q30：最终学习结论

ch5 对我来说最关键的收获是：操作系统不是简单“运行程序”，而是要把程序变成有身份、有上下文、有资源、有父子关系、有生命周期的对象来管理。

如果说 ch4 让我理解“每个程序有自己的地址空间”，那么 ch5 让我理解“每个地址空间背后都有一个进程身份，并且这个身份会参与创建、替换、等待、退出和调度”。

这也是从教学内核走向真实操作系统的一个重要分水岭。
