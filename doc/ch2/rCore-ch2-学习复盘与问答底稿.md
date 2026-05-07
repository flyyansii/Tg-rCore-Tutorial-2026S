# rCore ch2 学习复盘与问答底稿

## 1. 学习主线

ch2 的学习主线是：在 ch1 内核能启动的基础上，让内核具备运行用户程序的能力。

我一开始容易把它理解成“把多个程序放到内核里，然后一个个执行”。这个说法不算错，但不够完整。更准确地说，ch2 建立了三个机制：

```text
构建期：把用户程序编译并链接进内核。
运行期：内核把用户程序复制到指定地址并进入 U-mode。
交互期：用户程序通过 ecall 请求内核服务。
```

这三个机制合在一起，就是批处理操作系统的最小闭环。

## 2. 问题：批处理系统和普通函数调用有什么区别？

我的初始回答：

> 批处理就是一个程序执行完再执行下一个程序。

补充后的理解：

批处理不只是顺序执行。关键区别是：用户程序和内核程序处在不同特权级。用户程序不能直接调用内核函数，只能通过 `ecall` 陷入内核。内核处理完请求后，再决定是回到当前用户程序，还是结束当前程序并运行下一个。

所以 ch2 的批处理系统不是：

```text
kernel 调 app1()
kernel 调 app2()
```

而是：

```text
kernel 准备 app1 的用户上下文
  -> sret 进入 U-mode
  -> app1 ecall 回内核
  -> app1 exit
  -> kernel 准备 app2 的用户上下文
```

## 3. 问题：用户程序为什么不能直接 `println!`？

我的初始回答：

> 因为用户程序没有标准库，需要系统调用。

补充后的理解：

用户态 `println!` 最终需要输出到终端，而终端是硬件/内核资源。用户程序没有权限直接访问 SBI console 或 UART，所以 `println!` 必须封装成 `write` 系统调用。

调用链是：

```text
println!
  -> user_lib console
  -> sys_write
  -> ecall
  -> 内核 SyscallContext::write
  -> SBI/console 输出
```

所以 `println!` 不是消失了，而是从“标准库帮我们做”变成“我们自己的 user_lib + 内核 syscall 帮我们做”。

## 4. 问题：`ecall` 是什么？

我的初始回答：

> ecall 是调用内核级执行指令。

补充后的理解：

`ecall` 是 RISC-V 提供的环境调用指令。用户程序执行 `ecall` 时，CPU 会从 U-mode 切换到 S-mode，并跳到内核设置好的 Trap 入口。

它不是普通函数调用，因为普通函数调用不会改变特权级；`ecall` 会触发 Trap，让 CPU 进入更高特权级处理。

```text
普通函数调用：jal -> 同一特权级
系统调用：ecall -> U-mode 到 S-mode
```

## 5. 问题：TrapContext 是干什么的？

我的初始理解：

> 保存用户程序执行到哪里，类似 ARM 里的 LR 或 PC 返回信息。

补充后的理解：

这个类比方向是对的，但 TrapContext 保存得更多。它保存的是用户态被打断时的完整现场，包括通用寄存器、`sstatus`、`sepc` 等。

可以这样记：

```text
sepc：用户程序被打断的位置
sstatus：特权状态，决定 sret 后回到哪个模式
x[0..31]：用户程序当时用到的寄存器
```

如果没有 TrapContext，内核处理完系统调用后就不知道用户程序该从哪里继续、寄存器该恢复成什么。

## 6. 问题：`sepc` 为什么要加 4？

我的初始理解：

> 为了跳过 ecall。

补充后的理解：

RISC-V 中普通指令通常是 4 字节。发生 `ecall` 时，`sepc` 保存的是 `ecall` 这条指令的地址。如果处理完系统调用后不把 `sepc += 4`，`sret` 回去后会重新执行同一条 `ecall`，于是又陷入内核，形成死循环。

所以：

```text
write/yield 这种要返回用户程序的 syscall：
  需要 sepc += 4

exit 这种不返回当前程序的 syscall：
  不需要回到原来的 ecall 后面
```

## 7. 问题：`scause`、`sepc`、`sstatus` 分别是什么？

### `scause`

记录 Trap 原因，例如：

```text
UserEnvCall：用户执行 ecall
IllegalInstruction：非法指令
StoreFault：非法写地址
```

### `sepc`

记录 Trap 发生时用户程序的 PC。

### `sstatus`

记录特权状态。`sret` 会参考它决定返回后处于 U-mode 还是 S-mode。

你之前用 ARM 的 CPSR/APSR/LR 来类比，这是有帮助的：

```text
sepc 类似“异常返回地址”
sstatus 类似“异常前后的状态寄存器信息”
scause 类似“异常原因编号”
```

## 8. 问题：用户程序是怎么被链接进内核的？

我的初始回答：

> build.rs 生成 link_app.S，把用户程序放进内核。

补充后的理解：

构建期流程是：

```text
读取 cases.toml
编译用户程序
得到 ELF/bin
生成 app.asm
app.asm 用 .incbin 包含用户程序字节
main.rs 用 global_asm!(APP_ASM) 把 app.asm 链进内核
```

运行期再通过 `AppMeta::locate()` 找到这些程序的起止地址。

所以这里有两个阶段：

```text
构建期：用户程序被放进内核镜像。
运行期：内核从镜像中找到用户程序并复制到运行地址。
```

## 9. 问题：为什么用户程序需要 `_start`，不能只写 `main`？

我的初始回答：

> 因为需要定义执行顺序，不然 main 返回后不知道去哪。

补充后的理解：

用户程序运行前也需要一个最小运行时。它负责：

```text
清空 .bss
调用 main
拿到 main 返回值
调用 exit
```

如果直接从 `main` 开始，`main` 返回后就没有统一收尾逻辑。`_start` 的存在让用户程序有明确生命周期：

```text
_start -> main -> exit
```

## 10. 问题：`sys_write` 为什么不直接写在用户程序 main 里？

我的初始困惑：

> syscall 为什么写 main 里，为什么不都放 syscall.rs？

补充后的理解：

用户态和内核态各有一套 syscall 相关代码。

用户态：

```text
user/src/syscall.rs
  负责把 syscall id 和参数放进寄存器，然后 ecall。
```

内核态：

```text
ch2/src/main.rs 或 syscall 模块
  负责读取 syscall id，分发到 write/exit 等实现。
```

两者不是重复，而是一问一答：

```text
用户态 syscall.rs：发请求
内核态 SyscallContext：处理请求
```

## 11. 问题：ch2 和 ch3 的区别是什么？

ch2：

```text
一次只运行一个 app。
当前 app exit 后才运行下一个。
没有时间片。
没有任务轮转。
```

ch3：

```text
多个 app 都初始化为任务。
每个任务有 TCB。
可以 yield 主动让出 CPU。
可以靠时钟中断强制切换。
```

所以 ch2 是“批处理”，ch3 是“分时多任务”。

## 12. ch2 学完以后应该能讲清楚什么

我应该能讲清楚：

1. 用户程序如何在构建期被打包进内核。
2. 内核如何找到用户程序的二进制边界。
3. 内核如何把用户程序复制到运行地址。
4. 用户程序如何从 `_start` 调到 `main`。
5. `println!` 如何变成 `sys_write`。
6. `ecall` 如何从用户态进入内核态。
7. TrapContext 为什么要保存用户现场。
8. `sepc += 4` 为什么必要。
9. `exit` 为什么会让内核运行下一个程序。
10. 组件化仓库中哪些 crate 对应 Guide 原文中的 trap/syscall/batch 模块。
