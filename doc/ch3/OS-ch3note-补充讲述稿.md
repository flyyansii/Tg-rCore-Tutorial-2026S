# OS ch3 补充讲述稿

## 1. 为什么 ch3 要出现

ch2 的批处理系统有一个明显问题：如果当前程序不退出，后面的程序就永远运行不到。即使当前程序只是做大量计算，也会长时间占用 CPU。

ch3 的目标是让多个程序轮流运行。

```text
app0 运行一段
app1 运行一段
app2 运行一段
再回 app0
```

这就是分时多任务的雏形。

## 2. 多任务不是多个 CPU 同时跑

单核 CPU 同一时刻只能执行一个任务。ch3 的“多任务”是通过快速切换实现的。

```text
保存 app0
恢复 app1
保存 app1
恢复 app2
保存 app2
恢复 app0
```

只要切换足够快，人就感觉多个程序都在运行。

## 3. TCB：任务档案袋

内核想让一个任务暂停后还能继续，就必须记录这个任务的信息。TCB 就是 Task Control Block，任务控制块。

它至少要记录：

```text
任务上下文
任务是否结束
任务自己的用户栈
任务的系统调用计数
```

可以把 TCB 想成学生档案：

```text
姓名：哪个任务
状态：是否完成
书包：用户栈
进度条：上下文
统计表：syscall_count
```

## 4. TaskManager：管理所有任务档案

只有 TCB 还不够。系统里有很多任务，谁来决定下一个运行谁？这就是 TaskManager 的角色。

TaskManager 管：

```text
任务列表
当前任务编号
任务状态
调度策略
```

当前组件化仓库中，TaskManager 的逻辑被简化到主调度循环和任务数组中，但概念仍然存在。

它的核心问题是：

```text
从所有没 finish 的任务中，选一个继续运行。
```

## 5. yield：主动让出 CPU

`yield` 表示当前任务主动说：

```text
我先暂停一下，让别人跑。
但我以后还要回来。
```

所以 yield 不能把任务标记为结束。它只是触发调度器换下一个任务。

路径：

```text
用户 yield
  -> ecall
  -> 内核识别 SCHED_YIELD
  -> 保存当前任务现场
  -> 选择下一个任务
```

## 6. exit：任务真的结束

`exit` 和 `yield` 不同。

```text
yield：暂停，以后回来。
exit：结束，不再回来。
```

内核处理 exit 时，会把该任务标记为完成。之后调度器不会再选择它。

## 7. 时钟中断：内核强制切换

如果所有程序都很听话，经常 yield，那协作式调度就够了。但真实系统不能相信用户程序一定配合。

所以 ch3 引入时钟中断：

```text
时间片到
  -> CPU 自动 Trap 到内核
  -> 内核保存当前任务
  -> 切换下一个任务
```

这就让内核拥有强制夺回 CPU 的能力。

## 8. TrapContext：用户现场

当用户程序因为 ecall 或中断进入内核时，需要保存用户态现场。

这个现场就是 TrapContext。

它回答的问题是：

```text
我从用户程序哪里进来的？
用户程序当时寄存器是什么？
处理完后怎么回去？
```

关键字段：

```text
sepc：用户程序被打断的位置
sstatus：返回用户态所需状态
通用寄存器：用户程序计算现场
```

## 9. TaskContext：切换现场

TaskContext 和 TrapContext 不一样。

TrapContext 发生在用户态和内核态之间。TaskContext 发生在内核切换任务之间。

比如 app0 yield 进入内核后，内核决定切到 app1。这个“从 app0 的内核执行路径切到 app1 的内核执行路径”的过程，需要保存 TaskContext。

```text
TrapContext：保存用户程序现场。
TaskContext：保存内核切换现场。
```

## 10. 第一次进入程序为什么要伪造

第一次运行 app0 时，app0 从来没有被切出过，所以没有真实的 TaskContext。

但内核希望所有任务都走同一条恢复路径。于是它提前构造：

```text
一个初始 TrapContext
一个指向 __restore 的 TaskContext
```

这样第一次切换到 app0 时：

```text
__switch 恢复 TaskContext
  -> ret 到 __restore
  -> __restore 恢复初始 TrapContext
  -> sret 进入用户态
```

这就是“伪造上下文”的意义：让第一次启动看起来像一次普通恢复。

## 11. app0 为什么能回来

app0 被切走时，不是被丢掉了，而是保存了。

保存了两层：

```text
用户层：TrapContext
内核切换层：TaskContext
```

之后调度器重新选择 app0：

```text
恢复 app0 TaskContext
  -> 回到 app0 的恢复路径
  -> 恢复 app0 TrapContext
  -> sret 回用户态
```

所以 app0 能从上次暂停位置继续执行。

## 12. syscall 分发：Guide 和组件化仓库对照

Guide 中：

```text
syscall/mod.rs：总分发
syscall/fs.rs：write/read
syscall/process.rs：exit/yield
```

组件化仓库中：

```text
tg_syscall::handle：总分发
main.rs impl IO：write/read
main.rs impl Process/Scheduling：exit/yield
task.rs handle_syscall：从当前任务上下文中取 syscall id 和参数
```

所以虽然文件结构不同，但思想是一致的。

## 13. trace 作业的意义

trace 作业要求统计当前任务调用某个 syscall 的次数。

这迫使我们理解：

```text
系统调用发生在当前任务身上。
统计数据应该属于当前任务。
因此计数器应该放进 TCB。
```

`trace_request = 2` 查询次数时，也要把本次 trace 调用计入。

## 14. snake 扩展实验和 ch3 主线

贪吃蛇扩展不是为了炫图形，而是验证：

```text
用户态程序能跑游戏逻辑。
用户态通过 read/write 和内核交互。
内核能处理输入输出。
任务能 yield，不会卡死系统。
```

图形走：

```text
write(fd=3) -> graphics.rs -> VirtIO-GPU
```

输入走：

```text
VirtIO-keyboard -> keyboard.rs -> read(STDIN)
```

这依然遵守操作系统原则：用户程序不直接碰硬件。

## 15. 给别人讲 ch3 的顺序

我会这样讲：

1. ch2 批处理必须等当前程序 exit。
2. ch3 让多个任务轮流运行。
3. 每个任务需要一个 TCB。
4. TaskManager 管理所有 TCB 和状态。
5. yield 是主动切换，timer 是强制切换。
6. TrapContext 保存用户现场。
7. TaskContext 保存任务切换现场。
8. 第一次运行任务要伪造初始上下文。
9. `__switch` 保存旧任务并恢复新任务。
10. `__restore` 从 TrapContext 回到用户态。
11. syscall 会经过 fs/process 等语义分发。
12. trace 作业让我们把 syscall 计数放进 TCB。

## 16. 进一步细化：我会怎样把 ch3 讲给别人听

ch3 这章最难的地方，是它把“用户态和内核态切换”与“任务和任务切换”叠在了一起。为了讲清楚，我会拆成下面 35 个小步骤。

1. ch2 的批处理系统一次只运行一个程序。
2. 如果当前程序一直不 `exit`，后面的程序永远不能运行。
3. ch3 要解决这个问题：让多个程序轮流占用 CPU。
4. 单核上它不是真并行，而是并发。
5. 并发的本质是保存现场、切到别人、以后再恢复回来。
6. 内核首先要把每个 app 都包装成一个任务。
7. 每个任务都有一个 TCB，保存该任务的档案。
8. TCB 里有上下文、栈、是否结束等字段。
9. 组件化实验里还加入了 `syscall_count`，用于 trace 作业。
10. 多个 TCB 组成任务表。
11. 管理任务表和当前下标的逻辑就是 TaskManager。
12. TaskManager 要知道谁 Ready、谁 Running、谁已经 Exited。
13. 当前组件化版本不一定有独立 `TaskManager` 文件，但调度循环承担了这个角色。
14. 每个任务需要自己的用户栈。
15. 如果共用栈，app0 的局部变量可能被 app1 覆盖。
16. 第一次进入任务时，任务没有历史现场。
17. 内核提前构造初始 TrapContext。
18. 初始 TrapContext 里放用户入口、用户栈和返回 U-mode 的状态。
19. 内核还构造初始 TaskContext。
20. 初始 TaskContext 的 `ra` 指向 `__restore`。
21. 这样第一次 `__switch` 到该任务时，`ret` 会跳进 `__restore`。
22. `__restore` 从初始 TrapContext 恢复寄存器。
23. 最后 `sret` 进入用户态 app。
24. app 运行一段时间后可以主动 `yield`。
25. `yield` 通过 `ecall` 进入内核。
26. Trap 入口保存用户现场，这就是 TrapContext。
27. 内核根据 `a7` 判断 syscall id 是 `SCHED_YIELD`。
28. 内核把 `sepc += 4`，防止返回后重复 ecall。
29. 调度器决定切到下一个任务。
30. `__switch` 保存当前任务的 TaskContext。
31. `__switch` 恢复下一个任务的 TaskContext。
32. 如果下一个任务第一次运行，就走伪造的 `ra -> __restore`。
33. 如果下一个任务运行过，就回到它上次切走的位置。
34. timer interrupt 和 yield 类似，只不过不是用户主动让，而是内核强制抢。
35. 于是多个 app 就能交替推进，每个都能从上次暂停处继续。

## 17. ch3 最重要的两个上下文比喻

TrapContext 像“学生做题时桌面上所有草稿的快照”。老师突然叫停，学生离开座位，桌面状态要拍下来。回来时按快照恢复，才能继续做原来的题。

TaskContext 像“教室值班老师换班时交接的最小信息”。它不保存学生所有草稿，只保存老师自己回到工作流程所需的关键信息，比如从哪个流程继续。

所以：

```text
TrapContext 更靠近用户程序。
TaskContext 更靠近内核调度器。
```

## 18. ch3 中 syscall 的讲述顺序

以 `write/yield/exit/trace` 为例：

1. `write` 是 IO 类 syscall，对应 Guide 的 `fs.rs`。
2. `yield` 是调度类 syscall，对应 Guide 的 `process.rs`。
3. `exit` 是生命周期类 syscall，也对应 `process.rs`。
4. `trace` 是本课程练习扩展，用来观察当前任务的 syscall 历史。
5. 用户态永远只是发请求。
6. 内核态才真正决定打印、切换、结束或查询。

组件化版本中：

```text
tg_syscall::handle
  -> 按 syscall id 分发
  -> main.rs 中不同 trait 实现具体功能
  -> task.rs 把结果转换成 SchedulingEvent
```

这能解释为什么我们做 ch3 trace 时要改 `task.rs`：因为 syscall 计数属于“当前任务状态”，应该放进 TCB。
