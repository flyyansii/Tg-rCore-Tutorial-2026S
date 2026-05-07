# OS ch2 补充讲述稿

## 1. 从 ch1 到 ch2：内核开始服务用户程序

ch1 的内核像是“自己能开机、自己能打印”的裸机程序。ch2 进一步让它变成一个最小操作系统：它要能运行别的程序。

这一步看起来简单，其实已经出现操作系统最核心的边界：

```text
内核态：拥有硬件和特权资源。
用户态：运行普通应用，不能直接碰硬件。
系统调用：用户态请求内核服务的入口。
```

所以 ch2 的主线不是“多跑几个 hello world”，而是建立用户态和内核态之间的控制流。

## 2. 用户程序从哪里来

因为 ch2 还没有文件系统，所以用户程序不能像 Linux 那样从磁盘加载。它们是在构建内核时被一起打包进去的。

流程是：

```text
写 user/src/bin/xxx.rs
  -> 编译成 RISC-V 用户程序
  -> build.rs 读取 cases.toml
  -> 生成 app.asm
  -> app.asm 用 .incbin 嵌入用户程序二进制
  -> 内核编译时把 app.asm 链接进去
```

我可以把它理解为：ch2 的内核镜像里自带一包用户程序。内核启动后，不是去磁盘找程序，而是从自己镜像中的 app 区找。

## 3. app 在内核里和运行时不是一回事

这里最容易混淆：

```text
app 在内核镜像里的位置
  只是存放位置，像一个压在内核包里的文件。

app 运行时的位置
  是内核把它复制到 base 地址之后，真正从那里开始执行。
```

所以“链接进内核”和“运行在某个地址”是两步：

```text
先嵌入
再复制
最后执行
```

这也解释了为什么 `cases.toml` 里会有 base/step。它们不是告诉编译器源代码在哪里，而是告诉内核运行时把 app 放到哪里。

## 4. 批处理系统如何运行多个程序

批处理系统的逻辑很朴素：

```text
运行 app0
app0 exit
运行 app1
app1 exit
运行 app2
...
```

它没有分时，没有抢占，也没有“同时运行”。如果 app0 死循环不退出，app1 就永远不会被执行。

这和 ch3 的分时多任务不同。ch2 是顺序自动化，ch3 才是轮流切换。

## 5. 用户态到内核态的第一次闭环

当用户程序打印：

```rust
println!("Hello");
```

真正发生的是：

```text
println!
  -> user_lib::write
  -> syscall(SYS_WRITE, ...)
  -> ecall
  -> CPU 进入 S-mode
  -> 内核根据 scause 判断是 UserEnvCall
  -> 内核读取 a7 得到 syscall id
  -> 内核调用 write 实现
  -> 输出字符
  -> 设置返回值
  -> sepc += 4
  -> sret 回用户态
```

这个闭环是后面所有系统调用的基础。以后文件、进程、管道、线程，本质上也都要通过系统调用进入内核。

## 6. TrapContext 为什么关键

发生系统调用时，用户程序不是“正常调用内核函数”，而是被中断式地切到内核。

为了之后还能回去，内核必须保存用户现场。TrapContext 就是这个现场。

它保存：

```text
通用寄存器
sepc
sstatus
```

如果把用户程序比作正在写作业的人，TrapContext 就像拍了一张桌面照片：笔放哪里、纸写到哪、下一步该写哪一行，都被记录下来。内核处理完事情，再按照片恢复现场。

## 7. `sepc`、`scause`、`sstatus`

这三个 CSR 是理解 Trap 的核心。

`scause`：

```text
告诉内核为什么进来了。
比如 ecall、非法指令、访存错误。
```

`sepc`：

```text
记录用户程序在哪里被打断。
```

`sstatus`：

```text
记录特权级相关状态，影响 sret 回到哪里。
```

处理普通系统调用时，内核要把 `sepc` 向后移动一条指令，让用户程序回到 `ecall` 后面继续执行。

## 8. `sys_write` 和 `sys_exit` 的区别

`sys_write`：

```text
处理完以后，用户程序还要继续运行。
所以需要恢复现场，回到用户态。
```

`sys_exit`：

```text
用户程序已经结束。
所以内核不再回到这个 app，而是加载下一个 app。
```

这就是 ch2 批处理系统推进到下一个程序的原因。

## 9. 组件化仓库如何对应 Guide

Guide 原文中的模块比较展开，例如 `trap/`、`syscall/`、`batch.rs`。当前 tg 组件化仓库把这些拆到了 crate 或集中在 `main.rs` 中。

学习时可以这样对照：

```text
batch.rs 的 app 管理
  -> tg_linker::AppMeta + main.rs 批处理循环

trap.S / context.rs
  -> tg_kernel_context::LocalContext

syscall/mod.rs
  -> tg_syscall::handle

fs.rs/process.rs
  -> main.rs 中 impl IO / impl Process
```

这也是老师说“组件化 rCore”的意义：同样的 OS 原理，通过 crate 的方式复用和组织。

## 10. ch2-moving-tangram 如何理解

七巧板图形扩展不是 ch2 的主线，但它可以帮助理解批处理的节奏。

批处理本来是看不见的：

```text
app0 完成
app1 完成
app2 完成
```

图形化后可以变成：

```text
完成一个阶段，画出一块图形。
完成多个阶段，逐步拼出 OS 图案。
```

所以它是“用图形把批处理进度可视化”，不是替代 Trap/syscall 的核心学习。

## 11. 给别人讲 ch2 的顺序

如果我要讲给别人听，我会按这个顺序：

1. ch1 只是内核自己能启动，ch2 开始运行用户程序。
2. 用户程序构建期被打包进内核，因为还没有文件系统。
3. 内核运行时找到 app 的起止地址，把它复制到运行基址。
4. 内核构造用户上下文，用 `sret` 进入 U-mode。
5. 用户程序通过 `ecall` 请求内核。
6. TrapContext 保存用户现场。
7. 内核根据 `scause/a7` 判断系统调用类型。
8. `write` 会返回当前 app，`exit` 会切到下一个 app。
9. 这就是最小批处理系统。
10. 组件化版本把 Guide 中的 trap/syscall/batch 分散封装到了 crate 和 `main.rs`。
