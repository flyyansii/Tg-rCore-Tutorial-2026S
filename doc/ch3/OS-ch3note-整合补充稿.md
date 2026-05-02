# rCore ch3 学习笔记整合补充稿：多道程序、分时调度与图形贪吃蛇

本章从 ch2 的“批处理系统”继续往前走。ch2 中，内核一次只把一个用户程序放到固定地址运行，等这个程序 `exit` 之后，再加载下一个程序。ch3 的目标是让多个应用同时驻留在内存里，内核在它们之间来回切换，让它们看起来像“同时运行”。

我的理解是：ch2 更像一个自动执行脚本的机器，ch3 开始像一个真正的操作系统调度器。它不只是“运行程序”，而是开始“管理程序”。

## 1. ch3 到底新增了什么

ch3 的核心变化可以概括成三件事：

1. 多个应用同时加载到不同物理地址。
2. 每个应用都有自己的任务控制块 `TaskControlBlock`。
3. 内核通过 `yield`、时钟中断和轮转调度在任务之间切换。

在 ch2 中，应用都被加载到同一个地址，因此一次只能运行一个。ch3 通过 `base + app_id * step` 的方式把不同应用放到不同内存位置，例如：

```text
app0 -> 0x80400000
app1 -> 0x80600000
app2 -> 0x80800000
...
```

这样每个应用的代码不会互相覆盖，内核才能在它们之间轮流执行。

## 2. TaskControlBlock 是什么

`TaskControlBlock` 可以理解成“每个用户程序在内核里的档案袋”。它不是应用本身，而是内核为了管理应用额外维护的数据结构。

它主要保存：

```text
ctx: 用户态上下文，保存寄存器状态
finish: 任务是否已经结束
stack: 每个任务自己的用户栈
syscall_count: ch3 trace 练习添加的系统调用计数表
```

用 C 语言类比，它大概像：

```c
struct TaskControlBlock {
    Context ctx;
    bool finish;
    uint8_t stack[8192];
    size_t syscall_count[512];
};
```

它的作用是让内核能回答三个问题：

1. 这个任务执行到哪里了？
2. 这个任务结束了吗？
3. 下次恢复它时，寄存器和栈应该是什么状态？

## 3. 协作式调度与抢占式调度

ch3 中出现了两种切换方式。

协作式调度是用户程序主动让出 CPU。用户程序调用 `yield`，通过 `ecall` 进入内核，内核看到这是 `SCHED_YIELD`，就切换到下一个任务。

抢占式调度是内核强制打断用户程序。内核设置时钟中断，时间片到了之后，CPU 自动 Trap 进内核，内核保存当前任务状态，再切换到下一个任务。

我现在的理解是：

```text
yield: 用户说“我先让一下”
timer interrupt: 内核说“你时间片到了，先换别人”
```

这就是为什么 ch3 比 ch2 更像真实操作系统。真实系统不能完全相信应用主动让出 CPU，否则某个死循环程序就会霸占机器。

## 4. ch3 练习：sys_trace

ch3 的基础练习要求实现 `sys_trace`，系统调用号是 `410`。它有三种功能：

```text
trace_request = 0: 读取当前任务某个地址的 1 字节
trace_request = 1: 向当前任务某个地址写入 1 字节
trace_request = 2: 查询当前任务调用某个 syscall 的次数
其他值: 返回 -1
```

本章还没有虚拟地址空间隔离，所以这里的读写比较“裸”，直接把用户传来的地址当指针使用。它不安全，但符合本章教学目标：先理解系统调用、任务状态和计数，再到 ch4 引入地址空间后解决安全问题。

实现思路是：

1. 在 `TaskControlBlock` 中加入 `syscall_count: [usize; 512]`。
2. 在 `handle_syscall()` 里根据 `a7` 寄存器拿到 syscall id，并先计数。
3. 在 `trace_request = 2` 时返回当前任务记录的调用次数。
4. 通过 `caller.entity` 找回当前任务的 `TaskControlBlock`。

关键点是 `trace_request = 2` 要把本次 `trace` 调用也算进去，所以计数必须发生在真正分发系统调用之前。

## 5. ch3-snake：从系统调用到用户态游戏

扩展实验中，我们基于 ch3 做了用户态贪吃蛇。这次不是把游戏逻辑写死在内核里，而是让用户态程序负责游戏规则，内核只提供最小服务。

我们把它拆成两个版本：

```text
cargo run --features snake
交互版：QEMU 图形窗口中显示游戏，点击窗口后用 W/A/S/D 控制方向，Q 退出。

cargo run --features snake-ci
测试版：自动运行固定轨迹并退出，用于 CI 和脚本检查。
```

这样拆分是必要的。真正可玩的游戏会一直等待用户输入，不适合 CI；CI 需要一个自动退出的 demo，否则 GitHub Actions 会一直卡住。

## 6. 图形输出链路：fd=3 作为教学版 framebuffer 通道

普通 `write(STDOUT, buf)` 是输出字符。为了让用户态游戏画到 QEMU 图形窗口，我们增加了一个教学用的图形输出通道：

```text
用户态 ch3_snake.rs
  -> 打包 SnakeFrame
  -> write(fd = 3, frame_bytes)
  -> 内核 SyscallContext::write()
  -> graphics::submit_snake_frame()
  -> VirtIO-GPU framebuffer
  -> QEMU GTK 窗口显示彩色蛇、食物、边框和分数
```

这里 `fd = 3` 不是传统 Linux 的标准文件描述符，而是我们给 ch3 教学实验约定的一个“图形设备口”。这样设计的好处是：用户态仍然通过系统调用请求服务，内核决定如何把这个请求转换成硬件显示。

这也让我更清楚地理解了操作系统边界：

```text
用户态负责：游戏规则、蛇的位置、食物、方向、分数。
内核态负责：把用户给的帧数据画到 framebuffer。
```

## 7. 键盘输入链路：VirtIO Keyboard 到用户态 read

一开始尝试过 UART 终端输入，但终端刷新有残影，而且不适合图形游戏。后来改成 QEMU 的 `virtio-keyboard-device`。

输入链路如下：

```text
用户按 W/A/S/D/Q
  -> QEMU virtio-keyboard-device
  -> 内核 keyboard::refresh()
  -> VirtIOInput::pop_pending_event()
  -> Linux evdev keycode 转成 ASCII
  -> input::take()
  -> read(STDIN)
  -> user_lib::try_getchar()
  -> ch3_snake.rs 改变方向或退出
```

这仍然是轮询式输入，不是完整中断式键盘驱动，但它已经体现了“用户态不能直接碰设备，必须通过内核提供的抽象拿输入”。

## 8. 为什么 snake app 要放到 0x81000000

ch3 还没有 ch4 的地址空间隔离，所有东西本质上仍在同一个物理地址空间里。加入 VirtIO-GPU 后，内核里多了 framebuffer、DMA buffer、图形驱动状态等静态数据，内核占用的内存变大。

如果用户程序仍然放在较低地址，例如 `0x80400000`，就有可能和内核数据或设备 DMA 使用区域撞车。表现可能是：

```text
QEMU 打开了，但屏幕不显示
GPU setup framebuffer 失败
程序看似运行但没有图形
按键无响应
```

解决办法是在 `tg-rcore-tutorial-user/cases.toml` 中把 snake case 的应用基址调高：

```toml
[ch3_snake]
base = 0x8100_0000
step = 0x0020_0000
cases = ["ch3_snake"]
```

这不是最终操作系统的优雅方案。真正的解决办法会在 ch4 通过地址空间、页表和权限隔离来完成。但在 ch3 阶段，把扩展 demo 的 app 基址调高，是一个符合教学阶段的工程处理。

## 9. ch3 的整体闭环

ch3 的执行闭环可以这样理解：

```text
内核启动
  -> 加载多个 app
  -> 初始化每个 TaskControlBlock
  -> 选中 app0
  -> execute 进入用户态
  -> 用户程序运行
  -> ecall 或 timer interrupt 回到内核
  -> 内核保存/更新当前任务状态
  -> 选择下一个任务
  -> 再次 execute
```

对 snake 来说，闭环再多两条设备链：

```text
输入：virtio-keyboard -> kernel read -> user try_getchar
输出：user write(fd=3) -> kernel graphics -> virtio-gpu framebuffer
```

## 10. 本章我需要真正掌握的点

本章最重要的不是 Rust 语法，而是这些 OS 结构：

1. 用户程序为什么要分开放到不同地址。
2. TCB 为什么是任务管理的核心。
3. `yield` 和时钟中断的区别。
4. 系统调用是用户态到内核态的受控入口。
5. 内核如何根据 syscall id 分发到不同处理函数。
6. 为什么交互程序和 CI 程序需要拆开。
7. 为什么 ch3 的 trace 读写不安全，以及 ch4 为什么要引入地址空间。
8. 为什么图形游戏应该放在用户态，而不是把游戏逻辑塞进内核。
9. framebuffer 和 keyboard 都是设备服务，用户态应该通过 syscall 间接使用。

## 11. 本章测试结果

已经验证：

```text
cargo run
基础 ch3 多任务运行通过

cargo run --features exercise
ch3 trace 练习通过

cargo run --features snake-ci
ch3 snake 自动测试版通过

cargo build --features snake
ch3 snake 交互版编译通过
```

本地运行交互版：

```powershell
cd C:\Users\FLY\Desktop\OS\Tg-rCore-Tutorial-2026S-git\tg-rcore-tutorial-ch3
$env:Path="C:\Program Files\qemu;$env:Path"
cargo run --features snake
```

QEMU 图形窗口出现后，先点击窗口让键盘焦点进入 QEMU，然后：

```text
W: 上
A: 左
S: 下
D: 右
Q: 退出
```

演示 GIF 已保存为：

```text
doc/ch3/ch3-snake-terminal-demo.gif
```

桌面同步副本：

```text
C:\Users\FLY\Desktop\ch3-snake-terminal-demo.gif
```
