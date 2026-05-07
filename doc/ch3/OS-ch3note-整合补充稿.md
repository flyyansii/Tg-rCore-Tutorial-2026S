# OS ch3 分时多任务整合补充稿

## 一、本章总览

ch3 解决 ch2 批处理系统的局限。ch2 中，一个程序必须运行到 exit，内核才会运行下一个程序。ch3 引入多任务和分时调度，让多个任务可以轮流运行。

本章核心：

```text
保存当前任务现场
恢复另一个任务现场
让任务之后还能回来继续执行
```

这需要 TCB、TaskManager、TrapContext、TaskContext、yield、时钟中断等机制配合。

## 二、从批处理到多任务

ch2：

```text
app0 -> exit -> app1 -> exit -> app2
```

ch3：

```text
app0 运行一段
app1 运行一段
app2 运行一段
app0 继续运行
```

这并不是多个任务真正同时在一个 CPU 上跑，而是通过保存和恢复上下文实现并发。

## 三、任务控制块 TCB

TCB 是任务控制块。每个用户程序对应一个 TCB。

当前组件化仓库中的 TCB 包含：

```text
ctx：用户上下文
finish：是否结束
stack：用户栈
syscall_count：系统调用计数
```

TCB 让内核能回答：

```text
这个任务从哪里继续？
这个任务有没有结束？
这个任务自己的栈在哪里？
这个任务调用过多少次 syscall？
```

## 四、TaskManager

TaskManager 管理所有任务。

Guide 中它通常是一个单独模块，负责：

```text
保存任务数组
记录当前任务
维护 Ready/Running/Exited 状态
选择下一个任务
调用 __switch
```

当前组件化版本虽然没有单独 `task/mod.rs`，但主循环和任务数组承担了同样职责。

可以把关系理解为：

```text
TCB：一个任务的档案袋。
TaskManager：管理所有档案袋的调度员。
```

## 五、TrapContext 和 TaskContext

### TrapContext

TrapContext 保存用户态被打断时的现场。

发生在：

```text
ecall
异常
时钟中断
```

作用：

```text
处理完 Trap 后回到同一个用户程序。
```

### TaskContext

TaskContext 保存任务切换时的内核现场。

发生在：

```text
内核从 app0 切换到 app1
```

作用：

```text
以后能回到 app0 的内核恢复路径。
```

区别总结：

```text
TrapContext：用户态和内核态之间。
TaskContext：任务和任务之间。
```

## 六、第一次进入任务

第一次运行任务时，该任务没有历史上下文。内核需要提前构造一个初始现场。

Guide 中的典型逻辑：

```text
构造 TrapContext::app_init_context(entry, user_sp)
压入内核栈
构造 TaskContext::goto_restore()
ra 指向 __restore
```

第一次调度到该任务时：

```text
__switch
  -> 恢复 TaskContext
  -> ret 到 __restore
  -> __restore 恢复 TrapContext
  -> sret 进入用户态
```

这就是为什么可以用同一套恢复机制启动一个从未运行过的任务。

## 七、任务切换完整流程

以 app0 yield 切到 app1 为例：

```text
app0 用户态运行
  -> yield
  -> ecall
  -> 保存 app0 TrapContext
  -> 内核处理 syscall
  -> 返回 Yield 事件
  -> TaskManager 选择 app1
  -> __switch 保存 app0 TaskContext
  -> __switch 恢复 app1 TaskContext
  -> __restore 恢复 app1 TrapContext
  -> sret 进入 app1
```

之后 app1 yield 回 app0：

```text
保存 app1 TrapContext
保存 app1 TaskContext
恢复 app0 TaskContext
恢复 app0 TrapContext
app0 从上次暂停处继续
```

## 八、yield 与 exit

`yield`：

```text
任务主动让出 CPU。
任务没有结束。
保存现场，以后继续。
```

`exit`：

```text
任务结束。
标记 finish。
以后不再调度。
```

两者都通过 syscall 进入内核，但调度结果完全不同。

## 九、时钟中断

yield 需要用户程序主动配合。时钟中断让内核可以强制切换。

流程：

```text
内核设置 timer
用户程序运行
时间片到
SupervisorTimer 中断
保存当前 TrapContext
内核重新设置 timer
调度下一个任务
```

这让操作系统不再依赖用户程序是否自觉 yield。

## 十、syscall 分发链

用户态 syscall：

```text
user_lib
  -> syscall.rs
  -> ecall
```

内核态 syscall：

```text
TaskControlBlock::handle_syscall
  -> 从 a7 读取 syscall id
  -> 从 a0-a5 读取参数
  -> tg_syscall::handle
  -> IO/Process/Scheduling/Clock/Trace trait
```

Guide 中拆成：

```text
fs.rs：write/read
process.rs：exit/yield
mod.rs：总分发
```

组件化版本用 `tg_syscall` 和 trait 实现完成相同功能。

## 十一、trace 作业

trace 系统调用 ID 是 410。

三种功能：

```text
trace_request = 0：读取用户地址一个字节
trace_request = 1：写用户地址一个字节
trace_request = 2：查询某 syscall 调用次数
```

统计位置应该在 TCB：

```text
每个任务都有自己的 syscall_count。
handle_syscall 时先计数。
trace 查询当前任务自己的计数。
```

这能帮助理解“任务状态不仅包括寄存器，也包括该任务相关的运行统计信息”。

## 十二、ch3-snake 扩展

snake 扩展把 ch3 的机制用于用户态游戏。

用户态：

```text
维护蛇、食物、方向、游戏循环。
通过 read 获取输入。
通过 write(fd=3) 提交画面。
通过 yield 让出 CPU。
```

内核态：

```text
keyboard.rs 读取 VirtIO-keyboard。
graphics.rs 驱动 VirtIO-GPU。
main.rs 实现 read/write syscall。
调度循环保证游戏不会独占 CPU。
```

这说明图形游戏也应该通过系统调用请求内核服务，而不是直接访问硬件。

## 十三、关键寄存器和 CSR

`stvec`：

```text
Trap 入口地址。
```

`scause`：

```text
Trap 原因：ecall、timer、fault 等。
```

`sepc`：

```text
用户程序被打断的 PC。
```

`sstatus`：

```text
返回特权级和中断状态。
```

系统调用返回时：

```text
a0 写返回值。
sepc += 4。
sret 回用户态。
```

## 十四、和 ch2 的层层递进

ch2：

```text
用户态/内核态切换
系统调用
批处理顺序执行
```

ch3：

```text
多个任务状态
任务上下文保存恢复
yield 主动切换
timer 强制切换
```

所以 ch3 是建立在 ch2 Trap/syscall 基础上的。如果不理解 ch2 的 ecall 和 TrapContext，就很难理解 ch3 为什么能切走后再回来。

## 十五、本章一句话总结

```text
ch3 的核心是让每个用户程序都变成一个可暂停、可保存、可恢复的任务，并由内核调度器在多个任务之间轮转。
```
