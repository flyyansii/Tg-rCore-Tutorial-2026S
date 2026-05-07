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

## 十三、ch2 整合版细节清单：35 个必须能讲清楚的点

1. ch1 的内核只有自己，没有用户程序。
2. ch2 的目标是让内核运行外部用户程序。
3. 用户程序和内核不在同一特权级。
4. 用户程序处在 U-mode，内核处在 S-mode。
5. U-mode 不能直接访问内核函数和硬件。
6. 用户程序需要 `user_lib` 提供最小运行时。
7. 用户程序入口不是直接 `main`，而是 `_start`。
8. `_start` 负责清 `.bss`。
9. `_start` 负责调用 `main`。
10. `_start` 负责在 `main` 返回后调用 `exit`。
11. ch2 没有文件系统，所以 app 不能运行时从磁盘加载。
12. `build.rs` 在编译期工作，不是内核运行时工作。
13. `build.rs` 读取用户程序列表。
14. `build.rs` 编译用户程序。
15. `build.rs` 生成 app 链接汇编。
16. `.incbin` 把用户程序字节嵌入内核镜像。
17. `app_start/app_end` 表示 app 在内核镜像中的存放边界。
18. `base/step` 表示 app 被复制到的运行地址规则。
19. 内核启动后通过元数据找到 app。
20. 内核把 app 从镜像位置复制到运行位置。
21. ch2 没有页表，所以运行地址基本按物理地址理解。
22. 内核创建用户上下文，设置入口 PC。
23. 内核设置用户栈。
24. 内核设置 `sstatus`，让 `sret` 返回 U-mode。
25. 用户程序运行后如果打印，会调用用户态 `sys_write`。
26. `sys_write` 把 syscall id 放进 `a7`。
27. `sys_write` 把参数放进 `a0-a2`。
28. `ecall` 触发 U-mode 到 S-mode 的 Trap。
29. `stvec` 决定 Trap 入口。
30. `scause` 记录 Trap 原因。
31. `sepc` 记录用户程序被打断位置。
32. TrapContext 保存用户寄存器现场。
33. 内核 syscall 分发处理 `write/exit`。
34. `write` 处理完要 `sepc += 4` 并返回用户态。
35. `exit` 处理完不返回当前 app，而是进入下一个 app。

## 十四、Guide 代码树和组件化代码树的整合理解

Guide 拆得更细，适合教学理解；组件化仓库抽象得更强，适合复用。二者不是冲突关系。

```text
Guide batch.rs
  -> 教我“批处理系统应该有哪些职责”
  -> 当前仓库 main.rs/AppMeta 实现这些职责

Guide trap/context.rs + trap.S
  -> 教我“TrapContext 如何保存和恢复”
  -> 当前仓库 tg-kernel-context 封装这些细节

Guide syscall/fs.rs/process.rs/mod.rs
  -> 教我“系统调用应该按语义拆分”
  -> 当前仓库 tg-syscall + trait impl 完成分发

Guide link_app.S
  -> 教我“用户程序如何变成内核镜像里的数据”
  -> 当前仓库 build.rs 自动生成 app.asm
```

所以我读代码时应该先按 Guide 的结构在脑中建立地图，再去组件化仓库里找对应实现。

## 十五、ch2 的完整调用链总结

```text
构建期：
user app 源码
  -> user_lib 链接
  -> 编译成 RISC-V ELF/bin
  -> build.rs 生成 app.asm
  -> .incbin 嵌入内核
  -> 内核镜像携带 app 字节

运行期：
QEMU 启动内核
  -> 内核初始化
  -> AppMeta 找 app 边界
  -> 复制 app 到运行地址
  -> 构造用户上下文
  -> sret 进入 U-mode

系统调用期：
用户 println/exit
  -> user syscall wrapper
  -> ecall
  -> TrapContext 保存现场
  -> 内核 syscall 分发
  -> write 返回当前 app / exit 进入下一个 app
```

如果这一章只记一句话，我会记：

```text
ch2 不是简单地“跑几个程序”，而是第一次建立了用户程序和内核之间的特权边界、加载边界和系统调用边界。
```
