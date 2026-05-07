# rCore ch3 学习复盘与问答底稿

## 1. ch3 学习主线

ch3 的主题是多道程序与分时多任务。它在 ch2 批处理的基础上解决一个新问题：

```text
如果一个程序不退出，其他程序是不是永远不能运行？
```

ch2 的答案是“是”。ch3 的答案是“不一定”，因为内核可以保存当前任务现场，然后切换到另一个任务。

本章学习重点：

```text
TaskControlBlock
TaskManager
TrapContext
TaskContext
yield
timer interrupt
syscall 分发
```

## 2. 问题：ch2 和 ch3 到底差在哪里？

我的初始回答：

> ch2 是一个接一个，ch3 是同时执行。

修正后的答案：

ch3 不是多个程序真的同时在一个 CPU 上执行，而是快速轮流执行。单核情况下，同一时刻仍然只有一个任务占用 CPU，但因为切换很快，看起来像同时运行。

```text
ch2：app0 exit 后 app1 才开始。
ch3：app0 还没 exit，也可以 yield 或被 timer 打断，然后 app1 运行。
```

所以 ch3 更准确叫“并发”，不是“并行”。

## 3. 问题：为什么需要 yield？

我的初始理解：

> yield 是主动退出 CPU，但保存状态。

修正后的答案：

这个理解是对的。`yield` 不是退出程序，而是告诉内核：

```text
我现在可以先不运行，把 CPU 给别人。
但请保存我的状态，我之后还要继续。
```

它对应协作式调度。用户程序愿意配合内核，主动让出 CPU。

## 4. 问题：为什么还需要时钟中断？

我的初始理解：

> 因为有些程序不会主动 yield。

修正后的答案：

完全正确。如果只有 yield，一个恶意或写坏的程序可以永远不 yield，占住 CPU。时钟中断让内核拥有强制打断用户程序的能力。

```text
yield：用户主动让。
timer：内核强制抢。
```

这就是分时系统的基础。

## 5. 问题：TCB 是什么？

我的初始理解：

> TCB 是保存任务上下文和状态的结构体。

补充后的答案：

TCB 是任务控制块，可以理解成任务档案袋。它不仅保存上下文，还保存任务是否结束、用户栈、系统调用计数等信息。

当前组件化 ch3 中：

```text
TaskControlBlock {
    ctx: LocalContext,
    finish: bool,
    stack: [usize; 1024],
    syscall_count: [usize; 512],
}
```

其中：

```text
ctx：任务恢复时用的上下文。
finish：任务是否已经退出或被杀死。
stack：该任务自己的用户栈。
syscall_count：trace 作业统计系统调用次数。
```

## 6. 问题：TaskManager 是什么？

我的旧文档说得不够清楚。TaskManager 不是单个 TCB，而是管理所有 TCB 的系统。

它负责：

```text
保存所有任务的列表。
记录当前运行的是谁。
根据状态选择下一个任务。
处理 Ready/Running/Exited 等状态变化。
```

当前组件化仓库没有单独 `task/mod.rs`，但主循环 + TCB 数组承担了 TaskManager 的角色。

所以：

```text
TCB：一个任务的档案。
TaskManager：管理所有任务档案的人。
```

## 7. 问题：TrapContext 和 TaskContext 有什么区别？

这是你之前问得最关键的问题。

### TrapContext

保存用户态被打断时的现场。

发生在：

```text
用户 ecall
用户非法访存
时钟中断
```

它是用户态和内核态之间的桥。

### TaskContext

保存内核态切换任务时的现场。

发生在：

```text
内核决定从 app0 切到 app1
```

它是任务和任务之间的桥。

一句话：

```text
TrapContext 负责“从用户态回来还能回去”。
TaskContext 负责“从一个任务切走以后还能切回来”。
```

## 8. 问题：第一次进入 app 为什么要伪造上下文？

我的初始理解：

> 第一次没有正在运行的任务，所以给一个 unused，假装从空壳切到 app0。

修正后的答案：

方向正确，但还要补充：第一次进入 app 时，app 还没有真实被 Trap 过，也没有真实被 switch 过。为了复用统一的恢复路径，内核提前构造一个初始上下文。

Guide 中典型做法是：

```text
在内核栈上放一个初始 TrapContext。
TaskContext 的 ra 设置为 __restore。
第一次 switch 到该任务后，ret 会跳到 __restore。
__restore 恢复初始 TrapContext。
sret 进入用户态 app。
```

这就是所谓“伪造上下文”。

## 9. 问题：TrapContext 什么时候保存，什么时候改变？

保存时机：

```text
用户态发生 ecall、异常或中断时。
```

硬件和 Trap 入口配合把用户寄存器保存到 TrapContext。

改变时机：

```text
内核处理系统调用时可能修改 TrapContext。
```

比如：

```text
把返回值写入 a0。
把 sepc += 4。
```

这样恢复用户态后，用户程序能拿到 syscall 返回值，并从 `ecall` 后面继续执行。

## 10. 问题：TaskContext 什么时候保存，什么时候改变？

TaskContext 在 `__switch` 中保存和恢复。

从 app0 切到 app1：

```text
__switch(app0_task_cx_ptr, app1_task_cx_ptr)
  -> 把当前内核执行现场保存到 app0 的 TaskContext
  -> 从 app1 的 TaskContext 恢复寄存器
  -> ret 到 app1 的恢复路径
```

所以 TaskContext 的变化不是普通 Rust 赋值，而是在汇编 `__switch` 中自动发生。

## 11. 问题：为什么 app1 执行完还能回到 app0？

因为 app0 被切走时保存了两份东西：

```text
TrapContext：app0 用户态现场。
TaskContext：app0 内核态切换现场。
```

当之后调度器选择 app0：

```text
恢复 app0 TaskContext
  -> 回到 app0 上次切走后的内核恢复路径
  -> __restore 恢复 app0 TrapContext
  -> sret 回 app0 用户态
```

因此 app0 能从被打断的位置继续，而不是从头开始。

## 12. 问题：syscall 和 fs.rs/process.rs 的关系是什么？

Guide 里把系统调用分成：

```text
syscall/mod.rs：总分发
syscall/fs.rs：write/read
syscall/process.rs：exit/yield
```

组件化仓库里对应为：

```text
tg_syscall::handle：总分发
main.rs impl IO：相当于 fs.rs
main.rs impl Process/Scheduling：相当于 process.rs
task.rs handle_syscall：从 TCB 上下文中取 syscall id 和参数
```

所以看到组件化仓库没有 `fs.rs`，不能以为没有文件系统调用概念，而是被 trait 实现和 crate 封装了。

## 13. 问题：trace 作业为什么要改 TCB？

因为 trace 要统计“当前任务调用某个系统调用的次数”。

如果计数器放全局，就会把所有任务混在一起；放在 TCB 里，才能做到每个任务单独统计。

流程：

```text
handle_syscall 读取 syscall_id
  -> 当前 TCB 的 syscall_count[id] += 1
  -> trace_request=2 时返回当前 TCB 里的计数
```

注意：本次 trace 自己也要计入统计。

## 14. 本章学完后应该能讲清楚

1. 为什么 ch2 批处理不够。
2. yield 和 exit 的区别。
3. timer interrupt 为什么能强制切换。
4. TCB 保存什么。
5. TaskManager 管什么。
6. TrapContext 和 TaskContext 的区别。
7. 第一次进入 app 为什么要伪造上下文。
8. `__switch` 保存和恢复什么。
9. `__restore` 如何回到用户态。
10. app0 如何切到 app1 后还能回来。
11. syscall 如何经过 fs/process 分发。
12. trace 为什么要在 TCB 里统计。
