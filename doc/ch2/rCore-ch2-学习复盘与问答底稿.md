# rCore ch2 学习复盘与问答底稿

> 本文记录我在 ch2 学习和 ch2-moving-tangram 实验中的理解、误区修正、AI 协作过程和后续复习问题。

## 1. 我对 ch2 的一句话理解

ch2 让内核第一次真正成为“用户程序的执行环境”。它不只是自己启动，而是能把一批用户程序按顺序加载、运行、处理 syscall，并在一个程序结束后继续运行下一个程序。

```text
ch1：内核自己活起来。
ch2：内核开始运行别人，也就是运行用户程序。
```

## 2. 批处理不是多任务

我一开始容易把 ch2 和 ch3 混在一起。现在修正如下：

```text
ch2 批处理：
  app0 跑完 -> app1 跑完 -> app2 跑完
  没有时间片，没有任务切换。

ch3 多道/分时：
  app0 跑一会儿 -> app1 跑一会儿 -> app0 继续
  有任务上下文和调度。
```

所以 ch2 的重点不是“并发”，而是“自动顺序执行一批程序”。

## 3. 用户程序为什么也需要运行时

用户程序不是 Linux 上的普通程序，它也运行在我们自己写的内核环境里，因此也没有标准库、没有普通 main 启动流程。

用户程序启动链：

```text
user_lib::_start
  -> clear_bss
  -> main
  -> exit(main 返回值)
```

这里的 `main` 是用户写的逻辑，`_start` 是用户库提供的真正入口。`main` 返回后必须 `exit`，否则内核不知道当前 app 已经结束。

## 4. syscall 和 ecall 的关系

我的修正理解：

```text
syscall 是用户库里的函数封装。
ecall 是真正让 CPU 从 U-mode 进入 S-mode 的机器指令。
```

例如输出：

```text
println!
  -> write
  -> sys_write
  -> syscall(id, args)
  -> ecall
  -> 内核处理
```

用户态不能直接调用内核函数，因为那样特权级隔离就失效了。`ecall` 相当于受控入口，内核只处理它愿意暴露的服务。

## 5. 为什么要 sepc += 4

用户程序执行 `ecall` 后进入内核。此时 `sepc` 记录的是 `ecall` 这条指令的位置。

如果处理完 syscall 后直接 `sret`：

```text
CPU 回到 ecall 那一行
再次执行 ecall
再次进内核
无限循环
```

所以内核要：

```text
ctx.move_next()
```

也就是跳过当前 ecall，让用户程序从下一条指令继续。

## 6. AI 协作过程记录

本次 AI 协作主要分成五步。

第一步：理解任务。

```text
学：读 ch2，理解批处理、syscall、Trap。
教：整理代码链和文档，让自己以后能讲清楚。
用：基于 ch2 做 moving-tangram 图形扩展。
```

第二步：实现图形扩展。

```text
新增 virtio-drivers 依赖
新增 src/graphics.rs
修改 QEMU runner，打开 gtk + virtio-gpu-device
在 rust_main 批处理结束后调用 graphics::demo(completed_apps)
```

第三步：解决工具链问题。

```text
build.rs 需要 rust-objcopy
本地补充 cargo-binutils 和 llvm-tools-preview
```

第四步：解决 Rust 2024 unsafe 问题。

```text
tg-rcore-tutorial-syscall/src/user.rs
asm!("ecall") 必须显式放进 unsafe { ... }
```

这个修改不改变系统调用语义，只是适配新的 Rust unsafe 规则。

第五步：解决内存布局问题。

```text
加入图形模块后内核变大
旧 app 基址 0x8040_0000 不安全
改成 0x8100_0000
```

这一步最像真正 OS bug：不是语法错，而是地址布局冲突。

## 7. 图形扩展的验证状态

已验证内容：

```text
cargo build 通过
QEMU 能启动
app0-app7 能顺序运行
能进入 [ch2-tangram] init virtio gpu
图形窗口能显示 O/S 七巧板
右侧 S 被裁剪的问题已修正
```

需要诚实记录：

```text
当前进阶 demo 最后使用 spin_loop 保持图形窗口。
所以它不是自动退出型程序。
课程自带 test.sh 基础 checker 不适合直接跑当前图形版本。
如果后续要同时满足 checker 和 demo，建议加 feature：
  cargo run --features demo-graphics  保留窗口
  cargo run --features auto-exit      跑完自动 shutdown
```

目前实际采用的是更简单的 `ci` feature：

```text
普通 cargo run：保留图形窗口。
test.sh：cargo build --features ci，然后用 qemu -nographic 直接运行内核。
```

这样 GitHub Actions 不需要 GTK 图形环境，也不会被最后的 `spin_loop` 卡住。

## 8. 我需要能回答的问题

### Q1：ch2 和 ch3 的区别是什么？

ch2 是批处理，一个程序跑完再跑下一个。ch3 开始引入任务切换和调度，多个程序可以轮流执行。

### Q2：为什么 ch2 用户程序要打包进内核？

因为 ch2 还没有文件系统，内核不能运行时从磁盘读 app。只能在构建阶段用 `build.rs` 和 `.incbin` 把 app 二进制嵌进内核。

### Q3：为什么用户程序不能直接调用内核函数？

因为用户态权限低，不能随便访问内核态资源。必须通过 `ecall` 进入内核，由内核检查 syscall id 和参数。

### Q4：为什么加入图形后 app 基址要改？

因为图形驱动、DMA 缓冲区和绘图代码让内核占用空间变大，旧的 `0x8040_0000` 可能覆盖内核。把 app 装载到 `0x8100_0000` 可以避开冲突。

### Q5：为什么 framebuffer 写完还要 flush？

framebuffer 只是内存里的像素数组，`gpu.flush()` 才会让虚拟 GPU 把这块内存同步到图形窗口。

### Q6：为什么右侧 S 一开始缺一块？

QEMU 实际返回的显示分辨率是 `640x480`，而我最初按 `800` 宽度设计坐标。超过 `x=640` 的部分被裁剪了。修正方式是把右侧 S 的坐标压回 `410..630` 范围。

## 9. 我目前的阶段性结论

ch2-moving-tangram 对我来说不只是一个画图 demo。它把三个层面的知识串起来了：

```text
OS 原理：批处理、用户态/内核态、syscall。
工程实现：build.rs、app 打包、QEMU runner、VirtIO-GPU。
裸机调试：固定地址、内存覆盖、串口日志定位。
```

这次最重要的收获是：OS 实验里的 bug 不一定是 Rust 语法问题，很多时候是“机器实际怎么执行”和“内存实际怎么摆放”的问题。
