# rCore ch3 学习笔记整合补充稿：多道程序、分时调度、trace 与图形贪吃蛇

这一章是我真正开始感觉“操作系统不只是能跑一个程序，而是要管理很多程序”的地方。ch1 主要是在裸机上启动内核、输出字符；ch2 主要是在内核里嵌入多个用户程序，然后按批处理方式一个接一个执行；到了 ch3，系统开始允许多个应用同时驻留在内存里，并通过调度机制让它们轮流获得 CPU。

我一开始的直觉是：“多任务是不是就是很多程序同时跑？”后来在和 AI 讨论后修正为：单核 CPU 同一时刻仍然只能执行一条指令流，所谓“多个程序同时运行”，本质上是内核在多个任务之间快速切换。只要切换足够频繁，用户就会感觉它们像同时运行。

因此，ch3 的关键词不是“并行”，而是：

```text
多道程序加载
任务控制块 TCB
上下文保存与恢复
系统调用
时钟中断
分时调度
```

## 1. 从 ch2 到 ch3：批处理和多任务的差别

ch2 的批处理系统像这样：

```text
加载 app0 -> 运行 app0 -> app0 exit
加载 app1 -> 运行 app1 -> app1 exit
加载 app2 -> 运行 app2 -> app2 exit
```

它可以自动运行多个程序，但它仍然是串行的。每次真正运行的只有一个程序，而且下一个程序必须等上一个程序彻底结束后才能开始。

ch3 的多任务系统像这样：

```text
app0 跑一小段
切到 app1 跑一小段
切到 app2 跑一小段
再回到 app0
...
```

这里 app0 没有结束，它只是被暂停了。以后内核还能恢复它，让它从刚才暂停的位置继续执行。这就是我理解 ch3 的关键：内核必须拥有“保存现场”和“恢复现场”的能力。

## 2. 为什么要把不同应用放到不同地址

我一开始的回答是：“因为一个地址只能放一个程序，多个程序放一起会覆盖。”这个回答方向是对的，但可以说得更精确。

在 ch2 里，内核每次只运行一个应用，所以可以把所有应用都加载到同一个固定地址。反正 app0 跑完后，app1 覆盖 app0 也没关系，因为 app0 已经结束了。

但 ch3 不一样。ch3 要让多个应用同时驻留在内存里，app0 暂停后以后还要恢复，所以 app1 不能覆盖 app0 的代码和数据。于是内核需要把不同应用加载到不同物理地址：

```text
app0 -> 0x80400000
app1 -> 0x80600000
app2 -> 0x80800000
...
```

这一步体现的是多道程序的最基本要求：多个程序必须同时存在于内存中，否则谈不上在它们之间切换。

从工程角度看，本章还没有 ch4 的虚拟地址空间和页表，所以这里的“隔离”非常粗糙，只是靠不同物理地址区间避免直接覆盖。真正安全的隔离要到 ch4 引入地址空间后才能实现。

## 3. TaskControlBlock：任务在内核里的档案袋

`TaskControlBlock`，简称 TCB，可以理解成内核给每个任务建立的“档案袋”。用户程序本身只是一段代码和数据，但操作系统要管理它，就必须额外保存它的状态。

本章 TCB 里最重要的内容包括：

```text
ctx: 用户态上下文，保存寄存器状态
finish: 任务是否已经结束
stack: 每个任务自己的用户栈
syscall_count: ch3 trace 练习添加的系统调用计数表
```

用 C 语言类比，大概像：

```c
struct TaskControlBlock {
    Context ctx;
    bool finish;
    uint8_t stack[8192];
    size_t syscall_count[512];
};
```

我最开始容易混的是：TCB、上下文、栈、任务到底是什么关系？

后来整理成这样：

```text
任务 Task:
  一个正在被内核管理的用户程序执行实体。

TCB:
  内核为这个任务准备的管理结构。

Context:
  TCB 中最关键的一部分，记录寄存器现场。

Stack:
  任务运行函数调用和局部变量时需要的栈空间。
```

也就是说，TCB 是总档案袋，Context 和 Stack 是档案袋里的内容。内核调度任务时，不是直接“记住程序名字”，而是通过 TCB 知道这个任务执行到哪里、是否结束、下次怎么恢复。

## 4. 上下文保存与恢复：为什么任务能“暂停后继续”

一个程序被暂停时，如果只记录“它暂停了”是没用的。内核还必须记录它暂停那一刻 CPU 的状态：

```text
pc/sepc: 程序执行到哪条指令
sp: 栈指针在哪里
a0-a7: 参数、返回值、系统调用号
ra: 函数返回地址
其他通用寄存器: 当前计算的中间值
```

如果这些不保存，下次恢复时程序会“失忆”。比如一个程序刚算到循环第 10000 次，切出去以后回来，如果寄存器状态没了，它就不知道自己算到哪里。

所以我现在理解：

```text
暂停任务 = 保存上下文
恢复任务 = 恢复上下文
```

这个思想在后面章节会不断出现。ch3 的上下文保存还比较简化，后面到进程、地址空间、文件系统后，所谓“进程状态”会变得更复杂。

## 5. yield：用户主动让出 CPU

`yield` 是协作式调度的入口。用户程序主动调用 `sched_yield`，通过 `ecall` 进入内核，告诉内核：“我这轮先不跑了，你可以换别人。”

大致链路：

```text
用户程序 sched_yield()
  -> syscall
  -> ecall
  -> Trap 到内核
  -> TaskControlBlock::handle_syscall()
  -> 返回 SchedulingEvent::Yield
  -> 主调度循环选择下一个任务
```

我一开始的理解是：“yield 就是退出 CPU 但任务还没死。”这个理解是对的。更准确地说，yield 不会把任务标记为完成，它只是让当前任务暂时放弃本轮 CPU。以后调度器还会把它选回来。

对比 `exit`：

```text
yield:
  当前任务还活着，只是主动让出 CPU。

exit:
  当前任务已经结束，内核不再调度它。
```

## 6. 时钟中断：为什么不能只靠 yield

只靠 `yield` 有一个问题：它依赖用户程序自觉。如果某个用户程序写成：

```rust
loop {}
```

它永远不调用 yield，那么系统就会被它霸占。真实操作系统不能相信所有程序都会主动让出 CPU，所以需要时钟中断。

时钟中断的思路是：

```text
内核提前设置下一个时间点
  -> 用户程序运行
  -> 时间片到了
  -> CPU 自动 Trap 到内核
  -> 内核保存当前状态
  -> 切换到下一个任务
```

所以：

```text
yield 是用户主动让出 CPU。
timer interrupt 是内核强制收回 CPU。
```

这就是协作式调度和抢占式调度的区别。

我之前说过一句比较口语化的话：

```text
yield: 用户说“我先让一下”
timer interrupt: 内核说“你时间片到了，先换别人”
```

这个类比很适合自己记忆。

## 7. 系统调用：用户态进入内核态的受控入口

用户程序不能直接访问内核数据结构，也不能直接操作硬件。它如果想输出字符、退出、睡眠、读取输入，都要通过系统调用。

典型输出路径：

```text
println!
  -> user_lib::write
  -> syscall
  -> ecall
  -> Trap 到内核
  -> 内核根据 syscall id 分发
  -> SyscallContext::write
  -> SBI console 或图形设备
```

系统调用的核心价值是“受控”。用户程序不能随便跳到内核任意位置，只能通过内核开放的 syscall 接口请求服务。

这也解释了为什么后面做 snake 时，用户态游戏不能直接写 framebuffer 或读键盘 MMIO，而是要通过：

```text
read(STDIN)
write(fd=3)
sleep()
```

这些系统调用向内核请求服务。

## 8. ch3 基础练习：sys_trace

ch3 的基础练习要求实现 `sys_trace`，系统调用号是 `410`。它有三种功能：

```text
trace_request = 0:
  id 被当作 *const u8，读取该地址处 1 字节。

trace_request = 1:
  id 被当作 *mut u8，把 data 的低 8 位写入该地址。

trace_request = 2:
  id 被当作 syscall 编号，返回当前任务调用该 syscall 的次数。

其他 trace_request:
  返回 -1。
```

这道题训练的是两个点：

1. 在系统调用路径上统计每个 syscall 的调用次数。
2. 通过当前任务的 TCB 保存“每个任务自己的统计数据”。

为什么计数要放在 `TaskControlBlock` 里？

因为题目问的是“当前任务调用某个 syscall 的次数”。如果把计数放在全局变量里，多个任务的调用次数会混在一起，测试就不符合语义。

为什么要在 `handle_syscall()` 开头计数？

因为题目要求 `trace_request = 2` 时，本次 `trace` 调用也要计入统计。调用链应该是：

```text
用户调用 trace
  -> ecall
  -> handle_syscall 先 syscall_count[410] += 1
  -> 再进入 Trace::trace
  -> 查询 syscall_count[410]
```

如果先查询后计数，测试会少 1。

为什么 trace 的读写不安全？

因为 ch3 没有页表，内核无法可靠检查用户传来的地址是否合法。用户如果传一个乱地址，内核直接解引用就可能崩溃。这正好说明 ch4 引入地址空间的必要性。

## 9. ch3-snake：为什么游戏应该在用户态

扩展任务要求实现用户态贪吃蛇，并扩展内核能力支持它运行。

这句话里有两个重点：

```text
用户态贪吃蛇:
  游戏逻辑在用户程序里。

扩展内核能力:
  内核提供输入、输出、计时等机制。
```

也就是说，不能为了方便把蛇的移动、食物生成、分数计算都塞到内核里。那样虽然能显示游戏，但没有体现“应用程序使用操作系统服务”的结构。

最终设计：

```text
用户态 ch3_snake.rs:
  保存蛇的位置
  保存食物位置
  处理 W/A/S/D/Q 输入
  更新蛇移动
  打包 SnakeFrame
  调用 write(fd=3) 刷新画面

内核态 graphics.rs:
  接收 SnakeFrame
  检查 magic 和长度
  画边框、食物、蛇和分数
  flush 到 VirtIO-GPU framebuffer

内核态 keyboard.rs:
  轮询 VirtIO Keyboard 事件
  把 evdev keycode 转成 ASCII
  通过 read(STDIN) 交给用户态
```

这就是“内核提供机制，应用实现策略”。

## 10. 为什么不用终端 ASCII 版作为最终方案

最开始终端 ASCII 版更容易写，因为只要 `println!` 刷字符就行。但是实际运行时会有几个问题：

```text
终端残影明显
输入和输出都走串口，体验混乱
图形效果不像老师 demo
没有体现 framebuffer / GPU 设备链路
```

所以最终改成图形链路：

```text
用户态 write(fd=3, SnakeFrame)
  -> 内核 graphics.rs
  -> VirtIO-GPU framebuffer
  -> QEMU GTK 窗口
```

输入也改成 QEMU 图形窗口中的键盘事件：

```text
QEMU virtio-keyboard-device
  -> keyboard.rs
  -> input::take()
  -> read(STDIN)
  -> try_getchar()
  -> ch3_snake.rs
```

这样就更接近“用户态游戏 + 内核设备服务”的结构。

## 11. fd=3 是什么：教学版图形设备口

普通情况下：

```text
fd = 1: stdout
fd = 2: stderr/debug
```

本实验为了让用户态能提交图形帧，约定：

```text
fd = 3: graphics frame channel
```

用户态把游戏状态打包成 `SnakeFrame`，然后调用：

```rust
write(3, frame_bytes);
```

内核在 `SyscallContext::write()` 中判断 `fd == GRAPHICS_FD`，就不按普通字符处理，而是交给 `graphics::submit_snake_frame()`。

这个设计虽然不像完整 Linux 设备文件那么正规，但很适合 ch3 教学阶段：它让用户态仍然通过 syscall 请求服务，同时让我们能快速接上 framebuffer 图形输出。

## 12. VirtIO-GPU 和 VirtIO Keyboard 的作用

QEMU 模拟了一台 RISC-V 机器，也可以挂载 VirtIO 设备。我们这次给 ch3 加了两个设备：

```text
virtio-gpu-device:
  提供 framebuffer，用来显示图形。

virtio-keyboard-device:
  提供键盘事件，用来读取 W/A/S/D/Q。
```

内核侧新增：

```text
graphics.rs:
  初始化 GPU
  设置 framebuffer
  画蛇、食物、边框和分数

keyboard.rs:
  初始化键盘设备
  轮询按键事件
  把 keycode 转换为 ASCII
```

这一步让我把“设备驱动”理解得更具体：驱动不是神秘黑盒，它就是内核里一段知道怎么和某种硬件/虚拟硬件通信的代码。

## 13. 为什么 snake 的 app 基址调到 0x81000000

这个坑很关键。

ch3 还没有虚拟地址空间，所有东西都在一个物理地址空间里。加入图形后，内核里多了：

```text
GPU framebuffer 相关状态
DMA buffer
VirtIO 队列
键盘设备状态
全局 allocator
```

如果用户程序仍然放在较低的 `0x80400000` 附近，就可能和内核或设备缓冲区撞车。表现可能是：

```text
QEMU 窗口开了但没有画面
GPU setup framebuffer 失败
程序运行但图形不刷新
按键没有反应
```

解决办法是在 `cases.toml` 中单独把 snake 的用户程序基址调高：

```toml
[ch3_snake]
base = 0x8100_0000
step = 0x0020_0000
cases = ["ch3_snake"]
```

这不是最终的优雅方案，而是 ch3 阶段的工程处理。真正优雅的方案是 ch4 的地址空间和页表，把内核、用户程序、设备映射分清楚。

## 14. 为什么拆成 snake 和 snake-ci

交互版：

```text
cargo run --features snake
```

它应该一直运行，等待用户按 W/A/S/D/Q。如果它自动退出，就不是可玩的游戏。

CI 版：

```text
cargo run --features snake-ci
```

它必须自动跑固定帧数并退出。如果 CI 运行交互版，就会一直等用户输入，最后超时失败。

所以拆成两个版本是工程上必要的：

```text
snake:
  给人玩，强调交互体验。

snake-ci:
  给机器测，强调自动退出和可验证输出。
```

这也是我对“写给人用”和“写给 CI 测”差异的一次实际体会。

## 15. 本章测试和验证

本地验证过：

```text
cargo run
  ch3 基础多任务通过

cargo run --features exercise
  ch3 trace 练习通过

cargo run --features snake-ci
  ch3 snake 自动演示通过，输出 Test ch3 snake OK!

cargo build --features snake
  ch3 snake 交互版编译通过
```

由于 Windows 本地没有 WSL，`bash test.sh` 不能直接跑。但 GitHub/CNB 的 CI 是 Linux 容器，能正常执行 `test.sh`。为了避免本地 GTK QEMU 影响 CI，`test.sh` 中显式覆盖了 headless runner：

```text
qemu-system-riscv64
  -machine virt
  -display none
  -serial stdio
  -device virtio-gpu-device
  -device virtio-keyboard-device
```

这样本地运行 `cargo run --features snake` 会打开图形窗口，CI 运行测试则不会等待图形界面。

## 16. 我本章的收获

这一章对我来说最重要的不是 Rust 写法，而是把操作系统里几个概念串起来了：

```text
多道程序:
  多个 app 同时在内存里。

任务控制块:
  内核为每个 app 保存状态。

上下文切换:
  暂停和恢复任务的寄存器现场。

系统调用:
  用户态请求内核服务的入口。

时钟中断:
  内核强制收回 CPU 的机制。

用户态游戏:
  应用逻辑在用户态，内核提供输入输出机制。

设备支持:
  framebuffer 和 keyboard 都应该通过内核抽象给用户态使用。
```

如果要用一句话总结 ch3：

> ch3 让内核从“顺序执行多个程序”进化成“管理多个可暂停、可恢复的任务”，并通过系统调用和时钟中断构建了最小的分时多任务系统；在扩展实验中，用户态贪吃蛇进一步把输入、输出、计时和调度串成了一个真实可交互的应用场景。

## 17. 演示文件

交互操作演示 GIF：

```text
doc/ch3/ch3-snake-terminal-demo.gif
```

桌面同步副本：

```text
C:\Users\FLY\Desktop\ch3-snake-terminal-demo.gif
```
