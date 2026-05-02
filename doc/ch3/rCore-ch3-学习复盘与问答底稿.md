# rCore ch3 学习复盘与问答底稿

本文记录我和 AI 协作学习 ch3 时反复确认的问题。它不是标准答案摘抄，而是按“我容易混乱的点”整理出来的复盘。

## 1. 为什么 ch3 要把应用放到不同地址？

我的原始理解：

> 因为一个地址只能放一个程序，多个程序放一起会覆盖。

修正后的答案：

是的。ch2 中所有应用都加载到同一个位置，所以必须串行运行。ch3 要让多个程序同时驻留内存，因此每个程序需要不同加载地址。

```text
app0 -> base
app1 -> base + step
app2 -> base + 2 * step
```

这样内核才能在多个程序之间切换，而不是每次都重新覆盖加载。

## 2. TCB 和上下文有什么区别？

我的原始困惑：

> 上下文、任务、栈这些名字来回跳，感觉都差不多。

修正后的答案：

`TaskControlBlock` 是任务管理的总结构。它里面保存了用户上下文、任务状态、用户栈和统计信息。

可以这样理解：

```text
TaskControlBlock: 内核给一个任务建立的档案袋
LocalContext/ctx: 档案袋里最重要的一项，记录寄存器状态
stack: 这个任务自己的用户栈
finish: 这个任务是否已经结束
syscall_count: 这个任务自己的系统调用计数
```

不同教程版本中名字可能不同，但思想一样：必须保存“下次从哪里恢复”。

## 3. yield 和 exit 有什么区别？

我的原始回答：

> yield 是让出 CPU，exit 是进程结束。

修正后的答案：

这个理解是对的。更准确地说：

```text
yield:
  当前任务还活着，只是主动让出本轮 CPU。
  内核之后还会调度它。

exit:
  当前任务执行完毕。
  内核把它标记为 finish，不再调度它。
```

所以 `yield` 是暂停，`exit` 是结束。

## 4. 为什么只靠 yield 不够？

我的原始回答：

> 如果程序不主动 yield，就会一直占着 CPU。

修正后的答案：

对。协作式调度依赖用户程序“自觉”。但操作系统不能假设所有程序都自觉，否则一个死循环就能把系统卡死。

因此需要时钟中断：

```text
用户程序正在跑
  -> 时间片到
  -> CPU 自动 Trap 到内核
  -> 内核切换任务
```

这就是抢占式调度的基础。

## 5. 系统调用计数为什么要放在 TCB 里？

我的原始理解：

> 每个任务要统计自己的调用次数。

修正后的答案：

对。`sys_trace` 查询的是“当前任务”调用某个 syscall 的次数。如果放在全局变量里，多个任务会混在一起。

所以应该放在 `TaskControlBlock`：

```rust
syscall_count: [usize; 512]
```

每个任务一份，互不影响。

## 6. trace_request = 2 为什么本次调用也要计入？

因为题目明确要求“本次调用也计入统计”。所以计数必须发生在真正执行 `trace` 之前。

调用链大概是：

```text
用户调用 trace
  -> ecall
  -> handle_syscall 先 syscall_count[410] += 1
  -> 再进入 Trace::trace
  -> 查询 syscall_count[410]
```

如果顺序反过来，测试里的计数会少 1。

## 7. ch3 的 trace 读写为什么不安全？

我的原始理解：

> 因为现在没有地址空间隔离。

修正后的答案：

对。ch3 还没有页表，内核无法可靠判断用户传来的地址是否合法。用户传一个乱地址，内核直接解引用就可能崩溃。

这正好引出 ch4：

```text
ch3: 直接用裸地址，能跑测试但不安全
ch4: 引入页表和地址空间，检查地址是否合法
```

## 8. 贪吃蛇为什么要放在用户态？

我的理解：

> 游戏是应用程序，不应该直接塞进内核。

修正后的答案：

对。扩展实验的目标是“用户态游戏 + 内核支持”。也就是说，游戏逻辑应该在用户程序中：

```text
ch3_snake.rs:
  游戏循环
  蛇移动
  食物
  WASD 输入处理
  画面刷新请求
```

内核只提供最小服务：

```text
write(fd=3): 提交图形帧
read(STDIN): 读取键盘输入
clock_gettime/sleep: 控制帧率
yield/timer: 多任务调度
```

这更符合操作系统的边界：内核提供机制，应用实现策略。

## 9. 为什么从终端输入输出改成 VirtIO-GPU / VirtIO Keyboard？

我的原始困惑：

> 终端里 WASD 也能输入，为什么还要搞图形设备？

修正后的答案：

终端版适合快速验证，但不适合作为最终 demo：

```text
终端刷新会有残影
输入和输出混在同一串口
不像老师给的图形 demo
不能体现 framebuffer 和设备驱动
```

图形版更接近真实游戏应用：

```text
显示：用户态 write(fd=3) -> 内核 graphics.rs -> VirtIO-GPU framebuffer
输入：QEMU 窗口按键 -> keyboard.rs -> read(STDIN) -> 用户态 try_getchar
```

## 10. 为什么 snake 的用户程序基址要调高？

我的原始困惑：

> QEMU 能开，但画面不显示，是不是路径问题？

修正后的答案：

不完全是。ch3 没有地址空间隔离，图形驱动和 DMA buffer 增大了内核静态内存占用。如果用户程序还放在较低地址，就可能与内核或设备缓冲区发生覆盖。

因此对 snake case 单独设置：

```toml
[ch3_snake]
base = 0x8100_0000
step = 0x0020_0000
cases = ["ch3_snake"]
```

这属于 ch3 阶段的工程绕法。根本解决方案在 ch4：页表和地址空间隔离。

## 11. 为什么要拆成 snake 和 snake-ci？

这是工程上很重要的一点。

交互版：

```text
cargo run --features snake
```

它应该一直运行，等用户按键。否则游戏没法玩。

CI 版：

```text
cargo run --features snake-ci
```

它必须自动跑完并退出。否则 GitHub Actions 会一直等待，最后超时失败。

所以拆分不是偷懒，而是为了同时满足：

```text
人可以玩
机器可以测
```

## 12. 本次 AI 协作中踩过的坑

1. 一开始实现的是自动演示蛇，不是真正可操作版本。
2. 本地 Windows 找不到 `qemu-system-riscv64`，需要把 `C:\Program Files\qemu` 加进 PATH。
3. 不能把 Windows 绝对 QEMU 路径写进仓库配置，否则 Linux CI 会失败。
4. `TG_USER_LOCAL_DIR` 需要指向仓库内的 `../tg-rcore-tutorial-user`，否则 build.rs 会尝试 `cargo clone`。
5. 交互程序不能直接用于 CI，因为它会等待用户输入。
6. 终端 ASCII 游戏会有残影，图形 demo 更适合作为最终展示。
7. ch3 没有地址空间隔离，加入 GPU 后要注意内核静态数据和用户程序地址是否冲突。

## 13. 我应该能回答的检查问题

问题：ch3 相比 ch2 最大的变化是什么？

答案：ch3 支持多个应用同时驻留内存，并通过 TCB、系统调用和时钟中断在它们之间进行分时调度。

问题：为什么需要 TCB？

答案：因为内核要给每个任务保存独立状态，包括寄存器上下文、栈、结束标记和系统调用计数。

问题：yield 和 timer interrupt 的区别是什么？

答案：yield 是用户程序主动让出 CPU；timer interrupt 是内核通过硬件时钟强制打断当前程序。

问题：sys_trace 的系统调用计数为什么放在 `handle_syscall()` 里？

答案：因为所有 syscall 都会经过这里，且 trace 本次调用也要被计入。

问题：ch3-snake 的输入从哪里来？

答案：QEMU 的 VirtIO Keyboard 产生按键事件，内核 `keyboard.rs` 轮询读取并转成 ASCII，用户态通过 `read(STDIN)` 和 `try_getchar()` 拿到。

问题：ch3-snake 的图形输出走哪里？

答案：用户态把游戏状态打包成 `SnakeFrame`，调用 `write(fd=3)`，内核 `graphics.rs` 把它画到 VirtIO-GPU framebuffer。

问题：为什么要有 `snake-ci`？

答案：交互版会一直等用户按键，不适合自动测试；`snake-ci` 自动运行并退出，适合 CI 验证。
