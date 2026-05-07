# OS ch2 批处理系统整合补充稿

## 一、本章总览

ch2 是 rCore 从“裸机内核”走向“操作系统”的第一步。ch1 中内核只需要自己运行；ch2 中内核要作为用户程序的执行环境，负责加载、运行、服务和结束用户程序。

本章的核心闭环：

```text
构建期打包用户程序
  -> 内核运行时加载用户程序
  -> sret 进入用户态
  -> 用户程序 ecall 进入内核
  -> 内核处理系统调用
  -> 返回用户态或运行下一个程序
```

这就是批处理系统的基础。

## 二、用户程序构建与链接

ch2 还没有文件系统，所以用户程序必须在构建内核时提前嵌入内核镜像。

构建链：

```text
tg-rcore-tutorial-user/src/bin/*.rs
  -> cargo build 生成 RISC-V 用户程序
  -> ch2/build.rs 读取 cases.toml
  -> 生成 app.asm
  -> app.asm 用 .incbin 嵌入用户程序
  -> ch2/main.rs 用 global_asm!(APP_ASM) 链接进内核
```

`cases.toml` 决定本章要运行哪些用户程序，以及是否指定运行基址：

```text
[ch2]
base = 0x8100_0000
cases = [...]
```

这里的 `base` 表示运行时复制到的地址。它不是源文件地址，也不是 Rust 项目路径。

## 三、内核如何找到用户程序

构建生成的 app 元数据会被 `tg_linker::AppMeta` 定位。

运行时：

```text
AppMeta::locate()
  -> 得到 app 数量
  -> 得到每个 app 在内核镜像中的 start/end
  -> 计算大小
  -> 必要时复制到运行基址
```

这一步对应 Guide 中的 `link_app.S` 和 `batch.rs` 的功能。

可以理解成：

```text
app 在内核镜像里是“原材料”
复制到运行地址后才是“准备执行的程序”
```

## 四、用户程序启动

用户程序不是直接进入 `main()`。它有自己的最小运行时。

用户态启动链：

```text
内核 sret 到用户入口
  -> user_lib 启动代码
  -> clear_bss
  -> main()
  -> exit(main 返回值)
```

这样用户程序有明确生命周期：

```text
开始
  -> 执行 main
  -> main 返回
  -> 系统调用 exit
  -> 内核运行下一个 app
```

## 五、系统调用链

以 `println!` 为例。

用户态：

```text
println!
  -> user console
  -> sys_write
  -> syscall
  -> ecall
```

硬件：

```text
ecall
  -> U-mode 切 S-mode
  -> 跳到内核 Trap 入口
```

内核态：

```text
保存 TrapContext
  -> trap_handler/handle_syscall
  -> tg_syscall::handle
  -> SyscallContext::write
  -> console 输出
  -> 写返回值
  -> sepc += 4
  -> sret 回用户态
```

这条链要重点掌握，因为后面所有系统调用都沿用类似结构。

## 六、TrapContext 与 CSR

TrapContext 保存用户程序被打断时的现场。

关键 CSR：

```text
scause：为什么陷入内核
sepc：陷入内核前的用户 PC
sstatus：特权状态
```

系统调用处理完后，如果还要回到当前用户程序，就必须：

```text
设置 a0 为返回值
sepc += 4
sret
```

`sepc += 4` 的意义是跳过刚刚执行过的 `ecall`。

## 七、write 和 exit 的不同路径

`write` 路径：

```text
用户程序请求输出
  -> 内核打印
  -> 返回当前用户程序
```

`exit` 路径：

```text
用户程序请求结束
  -> 内核不再返回该程序
  -> 批处理系统加载下一个 app
```

所以 ch2 的“运行下一个程序”不是由用户程序自己跳过去，而是由内核在处理 `exit` 后决定。

## 八、组件化版本的模块对应

Guide 原始模块和当前仓库对应关系：

```text
Guide: batch.rs
当前: main.rs 批处理循环 + tg_linker::AppMeta

Guide: trap/context.rs, trap.S
当前: tg_kernel_context::LocalContext

Guide: syscall/mod.rs, fs.rs, process.rs
当前: tg_syscall crate + main.rs 中 SyscallContext

Guide: user/src/syscall.rs
当前: tg-rcore-tutorial-user/src/syscall.rs
```

当前仓库文件少，不代表概念少。很多底层细节被组件 crate 封装了。

## 九、ch2 与 ch3 的区别

ch2：

```text
一个程序完整运行到 exit。
没有时间片。
没有任务状态表。
没有主动/被动切换多个任务。
```

ch3：

```text
多个任务提前初始化。
每个任务有 TCB。
任务可以 yield。
时钟中断可以强制切换。
内核需要保存和恢复不同任务的上下文。
```

所以 ch2 是“批处理”，ch3 是“分时多任务”。

## 十、ch2-moving-tangram 实验理解

进阶实验用七巧板图形展示批处理结果。

它的意义：

```text
把多个 app 顺序执行的过程可视化。
把“批处理完成进度”映射成“图形逐块出现”。
```

本质上仍然是：

```text
用户程序顺序执行
  -> 内核统计进度
  -> 内核驱动 VirtIO-GPU 画图
```

图形部分不是 ch2 的基础主线，但能帮助我们把“批处理”从终端输出变成可观察的图像。

## 十一、我需要真正掌握的点

学完 ch2，我应该能回答：

1. 用户程序为什么要提前链接进内核？
2. app 在内核镜像中的地址和运行地址有什么区别？
3. `cases.toml` 的 base/step 是什么？
4. 用户态 `_start` 为什么要清空 bss 并调用 main？
5. `println!` 如何一步步变成 `sys_write`？
6. `ecall` 和普通函数调用有什么区别？
7. TrapContext 保存什么？
8. `scause/sepc/sstatus` 分别干什么？
9. 为什么 `sepc` 要加 4？
10. `exit` 为什么会让内核运行下一个程序？

## 十二、一句话总结

```text
ch2 建立了用户态和内核态之间的最小运行闭环：内核加载用户程序，用户程序通过 ecall 请求内核，内核处理后返回或切换到下一个程序。
```
