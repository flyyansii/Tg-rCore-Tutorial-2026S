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

## 15. 扩展问答：ch3 的 35 个细节问题

### 1. ch3 为什么说是“多道程序”？

因为内核同时维护多个用户程序的运行状态。虽然单核 CPU 同一瞬间只能执行一个任务，但内核可以在多个任务之间切换。

### 2. ch3 是并行吗？

不是。单核 QEMU 下更准确是并发。并行是多个 CPU 真同时执行；并发是快速切换，让多个任务都能推进。

### 3. ch2 的批处理哪里不够？

如果 app0 长时间不退出，app1 永远没机会运行。ch3 通过 `yield` 和 timer 解决这个问题。

### 4. `yield` 和 `exit` 的根本区别是什么？

`yield` 是暂停，保留现场；`exit` 是结束，释放或废弃现场。

### 5. 为什么有了 `yield` 还要 timer？

因为不能完全相信用户程序主动配合。timer 让内核拥有强制夺回 CPU 的能力。

### 6. TCB 是什么？

TCB 是一个任务的档案袋，里面放任务上下文、栈、状态、统计信息等。

### 7. TaskManager 是什么？

TaskManager 是任务档案管理员。它不是单个任务，而是管理所有 TCB、当前任务下标和调度策略。

### 8. 当前组件化仓库没有 `TaskManager` 文件怎么办？

不能说没有 TaskManager 概念。当前仓库把它拆到全局任务数组、调度循环和 `TaskControlBlock` 方法里了。

### 9. TCB 和 TaskManager 的关系？

TCB 是“一个任务的档案”，TaskManager 是“管理所有档案并决定谁运行的人”。

### 10. 每个任务为什么要有自己的用户栈？

函数调用、局部变量、返回地址都要用栈。如果多个任务共用栈，切换后数据会互相覆盖。

### 11. TrapContext 是保存用户现场的吗？

是。它保存用户态被 `ecall/中断/异常` 打断时的现场。

### 12. TaskContext 是保存用户现场的吗？

不是。TaskContext 保存的是内核态任务切换所需的现场，例如 `ra/sp/s0-s11`。

### 13. 为什么需要两种 Context？

因为有两种切换：用户态到内核态需要 TrapContext；内核在任务之间切换需要 TaskContext。

### 14. TrapContext 由谁保存？

在 Guide 中通常由 `trap.S::__alltraps` 保存；组件化版本中由 `tg-kernel-context::LocalContext` 封装了类似能力。

### 15. TaskContext 由谁保存？

由 `task/switch.S::__switch` 保存。它是汇编级别直接读写寄存器和内存。

### 16. 第一次进入任务为什么没有真实 TaskContext？

因为任务以前从没运行过，也就没有“上次被切走时的现场”。

### 17. 那第一次怎么运行？

内核伪造一个初始 TaskContext，让它的 `ra` 指向 `__restore`，再准备一个初始 TrapContext。

### 18. `ra -> __restore` 是什么意思？

`__switch` 恢复寄存器后会 `ret`，而 `ret` 跳到 `ra`。把 `ra` 设成 `__restore`，就能让第一次切换后自动进入恢复用户态的流程。

### 19. 初始 TrapContext 里有什么？

用户程序入口地址、用户栈指针、返回到 U-mode 所需的 `sstatus`，以及初始寄存器值。

### 20. 为什么说这是“骗系统”？

口语上像骗，因为任务没有真实历史现场；但本质是构造一个合法的初始现场，复用恢复路径。

### 21. app0 调用 yield 后第一件事是什么？

用户库把 syscall id 放到 `a7`，执行 `ecall`，CPU 切到 S-mode。

### 22. app0 的 TrapContext 什么时候保存？

进入 Trap 入口时保存。也就是执行 `ecall` 后、进入内核处理 syscall 前。

### 23. app0 的 TaskContext 什么时候保存？

调度器决定切到 app1，并调用 `__switch` 时保存。

### 24. app1 的 TaskContext 什么时候恢复？

同一次 `__switch` 中，在保存 app0 之后，从 app1 的 TaskContext 读取寄存器恢复。

### 25. 如果 app1 是第一次运行，恢复出来的是什么？

恢复出伪造的 `ra=__restore` 和指向初始 TrapContext 的栈位置。

### 26. 如果 app1 不是第一次运行呢？

恢复出 app1 上次被切走时保存的内核现场，然后继续走回用户态的恢复流程。

### 27. app1 后来怎么回到 app0？

app1 也会 yield 或被 timer 打断，然后 `__switch(app1, app0)` 恢复 app0 的 TaskContext。

### 28. app0 为什么不是从头开始？

因为 app0 的 TrapContext 保存了用户态 PC 和寄存器，TaskContext 保存了内核切换路径。

### 29. `sepc` 在 yield 中怎么变？

内核处理完 yield syscall 后要把 `sepc += 4`，否则 app0 以后恢复时会再次执行同一个 `ecall`。

### 30. `sstatus` 在恢复用户态中起什么作用？

`sret` 根据 `sstatus` 判断返回后的特权级。初始上下文必须设置成返回 U-mode。

### 31. `scause` 在 ch3 中有什么新作用？

除了识别 syscall，还要识别 timer interrupt、非法指令、访存异常等。

### 32. `stvec` 在 ch3 中有什么作用？

所有 Trap 都会跳到 `stvec` 指向的入口，这是内核接管用户程序的入口门牌号。

### 33. `syscall/fs.rs` 和 `syscall/process.rs` 为什么要拆？

这是为了语义清晰：文件/终端 IO 放 fs，任务生命周期控制放 process。组件化版本用 trait 替代了这种文件拆分。

### 34. trace 为什么要统计在 TCB 里？

因为 trace 统计的是“当前任务”的 syscall 历史。放全局会把所有任务混在一起。

### 35. ch3 最终比 ch2 多学到什么？

ch2 学会用户态和内核态往返；ch3 学会保存多个任务现场，让任务可切换、可暂停、可恢复。

## 16. 我对 ch3 的误区修正

1. 我一开始说“用户程序塞到内核级里”，更准确说法是：用户程序的二进制被嵌入内核镜像，但运行时仍然进入 U-mode。
2. 我一开始把多任务理解成同时运行，更准确是单核并发、快速切换。
3. 我容易把 TrapContext 和 TaskContext 都叫上下文，但它们解决的切换层次不同。
4. 我容易把第一次进入任务想成“直接跳 main”，实际上是伪造上下文后走 `__restore/sret`。
5. 我容易忽略 `TaskManager`，但没有它就无法管理所有任务状态。
6. 我容易把 `yield` 理解成退出，实际上它只是让出 CPU。
7. 我容易把 timer 理解成普通时间查询，其实 timer interrupt 是调度器强制切换的来源。
8. 我容易只看当前组件化文件，忘记 Guide 中 `loader/task/trap/syscall` 分层是理解逻辑用的地图。

## 17. 给自己讲 ch3 的推荐顺序

1. 先回顾 ch2 的限制：一个 app 不退出，后面的 app 没机会。
2. 引入 TCB：每个 app 都要有自己的任务档案。
3. 引入 TaskManager：内核要管理所有任务，而不是只管理当前 app。
4. 解释 TrapContext：用户态被打断时保存现场。
5. 解释 TaskContext：内核态切换任务时保存现场。
6. 用第一次运行解释伪造上下文。
7. 用 app0 yield 到 app1 解释 `__switch`。
8. 用 app1 yield 回 app0 解释为什么能恢复。
9. 用 timer interrupt 解释为什么可以强制分时。
10. 用 trace 作业解释为什么 syscall 统计要放在 TCB。

## 18. 非问答版流程复盘：ch3 的 30 步学习链

这一段把 ch3 的理解过程写成连续流程，而不是问答列表。

1. 我先从 ch2 的限制出发：一个程序不退出，后面的程序就运行不了。
2. ch3 的目标是让多个程序都能推进。
3. 内核把每个用户程序包装成一个任务。
4. 每个任务都需要一个 TCB。
5. TCB 保存该任务自己的上下文、栈和状态。
6. 多个 TCB 组成任务表。
7. TaskManager 或调度循环负责管理任务表。
8. 内核初始化时为每个 app 创建 TCB。
9. 每个 TCB 都设置自己的用户栈。
10. 每个 TCB 都设置自己的初始用户入口。
11. 第一次运行任务时，没有历史上下文。
12. 内核提前构造初始 TrapContext。
13. 内核提前构造初始 TaskContext。
14. 初始 TaskContext 让第一次恢复路径指向 `__restore`。
15. 调度器选中第一个任务。
16. `__switch` 进入第一个任务的恢复路径。
17. `__restore` 恢复初始 TrapContext。
18. `sret` 进入用户态。
19. 用户程序运行并可能调用 `yield`。
20. `yield` 执行 `ecall` 回到内核。
21. Trap 入口保存用户现场。
22. 内核识别这是调度类 syscall。
23. 内核不结束当前任务，只把它放回可运行集合。
24. 调度器选择下一个任务。
25. `__switch` 保存当前任务 TaskContext。
26. `__switch` 恢复下一个任务 TaskContext。
27. 下一个任务进入用户态运行。
28. timer interrupt 可以在用户不主动 yield 时强制触发同样的切换。
29. 当任务 exit 或异常退出时，内核把它标记为结束。
30. 所有任务不断经历“运行、陷入、调度、恢复”，形成分时多任务系统。

## 19. ch3 流程中最容易断掉的三根线

第一根线是用户态到内核态：

```text
用户 app
  -> ecall/timer/fault
  -> TrapContext
  -> trap handler
```

第二根线是内核态任务切换：

```text
trap handler
  -> 调度器
  -> __switch
  -> TaskContext
```

第三根线是回到用户态：

```text
TaskContext 恢复
  -> __restore
  -> TrapContext 恢复
  -> sret
  -> 用户 app 继续
```

如果这三根线能连上，ch3 的主体流程就不会乱。
