# rCore ch3 代码链与模块对应底稿

## 0. ch3 主线

ch2 是批处理：一个 app exit 后才运行下一个。ch3 的目标是分时多任务：多个 app 都可以被内核管理，每个 app 运行一小段时间后让出 CPU，之后还能从原来的位置继续。

核心问题：

```text
如何保存一个任务的执行现场？
如何恢复另一个任务的执行现场？
第一次运行任务时没有旧现场怎么办？
系统调用和时钟中断如何触发调度？
```

Guide 原文中的 ch3 通常有：

```text
loader.rs
task/
  context.rs
  switch.S
  switch.rs
  task.rs
  mod.rs
timer.rs
trap/
syscall/
```

当前组件化 `tg-rcore-tutorial-ch3` 更集中：

```text
tg-rcore-tutorial-ch3/
├── src/main.rs
├── src/task.rs
├── src/graphics.rs
└── src/keyboard.rs
```

对应关系：

```text
Guide loader.rs
  -> build.rs + tg_linker::AppMeta + main.rs 加载循环

Guide task/task.rs
  -> ch3/src/task.rs::TaskControlBlock

Guide task/mod.rs TaskManager
  -> ch3/src/main.rs 中全局任务数组 + 调度循环

Guide task/context.rs + switch.S
  -> tg-kernel-context crate 的 LocalContext::execute

Guide trap/mod.rs
  -> main.rs 调度循环读取 scause 并调用 handle_syscall

Guide syscall/fs.rs/process.rs
  -> tg-syscall crate + main.rs 中 IO/Process/Scheduling/Clock/Trace trait 实现
```

## 1. ch3 启动和任务初始化链

```mermaid
flowchart TD
    A["QEMU 启动内核"] --> B["main.rs::_start"]
    B --> C["rust_main()"]
    C --> D["zero_bss / init_console / init syscall"]
    D --> E["AppMeta::locate().iter()"]
    E --> F["为每个 app 创建 TaskControlBlock"]
    F --> G["TaskControlBlock::init(entry)"]
    G --> H["LocalContext::user(entry)"]
    H --> I["设置用户 sp 到该任务用户栈顶"]
    I --> J["finish=false; syscall_count 清零"]
    J --> K["全部任务进入调度循环"]
```

`TaskControlBlock` 可以理解成一个任务档案袋：

```text
ctx：用户态上下文，保存寄存器和返回位置
finish：任务是否结束
stack：该任务自己的用户栈
syscall_count：trace 作业用的系统调用计数
```

Guide 里的 `TaskManager` 在当前组件化仓库中没有单独文件，但功能仍然存在：全局任务数组、当前下标、轮转选择未完成任务，这些共同承担了 TaskManager 的职责。

## 2. TaskManager 的职责

TaskManager 不是只保存 TCB，它要管理任务状态。

Guide 里的典型状态：

```text
UnInit
Ready
Running
Exited
```

当前组件化版本简化成：

```text
finish = false：还能运行
finish = true：已经 exit 或被杀死
当前下标 i：调度器正在考虑哪个任务
```

调度器做的事：

```mermaid
flowchart TD
    A["从当前 i 开始"] --> B{"task[i].finish?"}
    B -- "true" --> C["i = next"]
    C --> B
    B -- "false" --> D["执行 task[i]"]
    D --> E{"返回事件"}
    E -- "Yield/Timer" --> F["i = next"]
    E -- "Exit/Fault" --> G["finish = true"]
    G --> F
    F --> A
```

所以你可以把 TaskManager 理解成“任务表 + 状态机 + 选下一个任务的策略”。

## 3. TrapContext 和 TaskContext 的区别

这是 ch3 最容易混的点。

### TrapContext

TrapContext 保存“用户态进入内核时”的现场。

触发场景：

```text
用户程序 ecall
用户程序非法指令
用户程序访存异常
时钟中断
```

保存内容：

```text
用户通用寄存器
sepc
sstatus
```

作用：

```text
让内核处理完 Trap 后，还能回到同一个用户程序继续。
```

### TaskContext

TaskContext 保存“内核态任务切换时”的现场。

触发场景：

```text
内核决定从 app0 切到 app1
```

保存内容通常更少：

```text
ra
sp
s0-s11 等 callee-saved 寄存器
```

作用：

```text
让内核以后能回到某个任务对应的内核执行路径。
```

一句话区分：

```text
TrapContext：用户态 <-> 内核态之间的现场。
TaskContext：内核态任务 <-> 内核态任务之间的现场。
```

当前组件化版本中，这些底层细节被 `LocalContext` 封装，但理解上仍然沿用这个区别。

## 4. 第一次进入任务：为什么要“伪造”上下文

你之前问过：第一次运行 app 时，它明明没有被切出过，为什么可以被“恢复”？

答案是：内核提前构造了一个看起来像“刚从内核返回用户态”的上下文。

Guide 中常见做法：

```text
TaskContext::goto_restore()
  -> ra 设置成 __restore
  -> sp 指向内核栈上提前放好的 TrapContext
```

这样第一次 `__switch` 到该任务时：

```text
__switch 恢复 ra/sp
  -> ret 跳到 __restore
  -> __restore 从伪造的 TrapContext 恢复用户寄存器
  -> sret 进入 app0 用户态
```

所以“假地址骗系统运行”的本质是：

```text
第一次没有真实的历史现场。
内核就提前摆好一个初始现场。
让通用的恢复路径以为它正在恢复一个任务。
```

这不是作弊，而是操作系统常用技巧：用统一的上下文切换路径启动新任务。

## 5. app0 yield 后如何回到 app1，再回到 app0

```mermaid
flowchart TD
    A["app0 用户态运行"] --> B["app0 调用 yield"]
    B --> C["ecall 进入内核"]
    C --> D["保存 app0 TrapContext"]
    D --> E["trap_handler/syscall 识别 yield"]
    E --> F["调度器选择 app1"]
    F --> G["__switch(app0_task_cx, app1_task_cx)"]
    G --> H["保存 app0 TaskContext"]
    H --> I["恢复 app1 TaskContext"]
    I --> J{"app1 第一次运行?"}
    J -- "是" --> K["ra -> __restore"]
    K --> L["__restore 恢复 app1 初始 TrapContext"]
    J -- "否" --> M["返回 app1 上次切出点"]
    L --> N["sret 进入 app1 用户态"]
    M --> N
```

之后 app1 再 yield：

```text
app1 保存自己的 TrapContext
__switch 保存 app1 TaskContext
恢复 app0 TaskContext
回到 app0 上次被 switch 走之后的位置
__restore 恢复 app0 TrapContext
sret 回 app0 用户态
```

这就是为什么 app0 运行一半后还能继续：它的用户现场在 TrapContext 中，内核切换现场在 TaskContext 中。

## 6. syscall 调用链：以 write 为例

```mermaid
flowchart TD
    A["用户程序 println!"] --> B["user_lib::write"]
    B --> C["user syscall.rs::syscall"]
    C --> D["ecall"]
    D --> E["硬件进入 S-mode"]
    E --> F["保存 TrapContext"]
    F --> G["main.rs 调度循环读 scause"]
    G --> H["task.rs::TaskControlBlock::handle_syscall"]
    H --> I["读取 a7 syscall id"]
    I --> J["读取 a0-a5 参数"]
    J --> K["tg_syscall::handle"]
    K --> L["IO::write / fs.rs 语义"]
    L --> M["console 输出"]
    M --> N["返回值写回 a0"]
    N --> O["sepc += 4"]
    O --> P["返回 SchedulingEvent::None"]
    P --> Q["继续执行当前任务"]
```

Guide 中会把 syscall 拆成：

```text
syscall/mod.rs：按 syscall id 分发
syscall/fs.rs：write/read 等文件 IO
syscall/process.rs：exit/yield 等进程控制
```

组件化版本中：

```text
tg_syscall::handle：统一分发
main.rs impl IO：对应 fs.rs
main.rs impl Process/Scheduling：对应 process.rs
task.rs handle_syscall：从上下文取 id/args，并把返回事件交给调度器
```

## 7. yield 调用链

```mermaid
flowchart TD
    A["用户程序 yield"] --> B["ecall"]
    B --> C["TaskControlBlock::handle_syscall"]
    C --> D["tg_syscall::handle"]
    D --> E["Scheduling::sched_yield"]
    E --> F["返回 Ret::Done"]
    F --> G["识别 Id::SCHED_YIELD"]
    G --> H["ctx.move_next()"]
    H --> I["返回 SchedulingEvent::Yield"]
    I --> J["主调度循环选择下一个未完成任务"]
```

`yield` 的含义不是退出，而是：

```text
我暂时让出 CPU，但我的状态要保存，之后还要回来。
```

## 8. exit 调用链

```mermaid
flowchart TD
    A["用户程序 exit"] --> B["ecall"]
    B --> C["TaskControlBlock::handle_syscall"]
    C --> D["识别 Id::EXIT"]
    D --> E["返回 SchedulingEvent::Exit(code)"]
    E --> F["主循环设置 tcb.finish = true"]
    F --> G["不再恢复该任务"]
    G --> H["选择下一个未完成任务"]
```

`exit` 和 `yield` 的区别：

```text
yield：保存现场，以后回来。
exit：任务结束，不再回来。
```

## 9. 时钟中断和分时

ch3 从“协作式 yield”进一步走向“分时”。时钟中断让任务即使不主动 yield，也会被内核打断。

```mermaid
flowchart TD
    A["设置下一次 timer"] --> B["用户任务运行"]
    B --> C["SupervisorTimer 中断"]
    C --> D["保存 TrapContext"]
    D --> E["内核识别 scause=SupervisorTimer"]
    E --> F["重新设置 timer"]
    F --> G["返回调度循环"]
    G --> H["选择下一个任务"]
```

这里 `stvec` 指向 Trap 入口，`scause` 告诉内核这是时钟中断，`sepc` 保存被打断的用户 PC，`sstatus` 保存返回状态。

## 10. trace 作业调用链

```mermaid
flowchart TD
    A["用户 ch3_trace.rs"] --> B["trace_read/trace_write/count_syscall"]
    B --> C["syscall id = 410"]
    C --> D["TaskControlBlock::handle_syscall"]
    D --> E["先 syscall_count[id] += 1"]
    E --> F["tg_syscall::handle"]
    F --> G["main.rs::Trace::trace"]
    G --> H{"trace_request"}
    H -- "0" --> I["读用户地址 1 字节"]
    H -- "1" --> J["写用户地址 1 字节"]
    H -- "2" --> K["查询当前 TCB 的 syscall_count"]
```

这说明 trace 的统计应该放在 TCB 里，因为每个任务有自己的 syscall 历史。

## 11. ch3-snake 扩展和基础主线

snake 不是 ch3 基础机制本身，而是用用户态游戏检验：

```text
多任务
系统调用
输入输出
分时调度
```

图形输出：

```text
用户态 SnakeFrame
  -> write(fd=3)
  -> 内核 graphics.rs
  -> VirtIO-GPU
```

键盘输入：

```text
VirtIO-keyboard
  -> keyboard.rs
  -> input::take
  -> read(STDIN)
  -> 用户态改变方向
```

这和 Guide 的基础目标一致：用户程序仍然只通过系统调用和内核交互。

## 12. ch2 到 ch3 的本质升级

```text
ch2：内核每次只关心一个 app。
ch3：内核同时维护多个任务的档案和状态。

ch2：exit 后才进入下一个。
ch3：yield 或 timer 后就能切换。

ch2：只需要一个当前用户上下文。
ch3：每个任务都要有自己的上下文和栈。
```

一句话：

```text
ch3 的本质是把“程序顺序执行”升级成“任务状态可保存、可切换、可恢复”。
```
