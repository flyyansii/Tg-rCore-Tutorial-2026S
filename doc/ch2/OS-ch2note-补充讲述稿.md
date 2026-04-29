# OS ch2 补充讲述稿：从批处理系统到 moving tangram

> 这一稿尽量按“我讲给同学听”的口吻写，不追求像教材，而是追求把我真正理解的过程讲清楚。

## 1. 为什么第二章不是“又写一个 Hello world”

第一章的 Hello world 解决的是内核自己的启动问题：裸机上没有标准库，没有普通 main，没有操作系统帮我打印，所以我要自己搭一个最小运行环境。

第二章开始，问题变了。内核不只是自己运行，它要运行用户程序。也就是说，内核开始承担操作系统最基本的职责：给应用程序提供执行环境。

我可以这样理解：

```text
第一章：我让内核自己站起来。
第二章：我让内核带着用户程序跑起来。
```

## 2. 批处理系统到底是什么

批处理的意思不是多个程序同时跑，而是内核自动执行一批程序。

```text
app0 运行
app0 退出
app1 运行
app1 退出
app2 运行
...
```

它的价值是自动化。以前如果只有一个程序，运行完就结束；批处理让内核能提前拿到一批程序，然后一个接一个执行。

这比第一章更像操作系统了，但还不是现代意义上的多任务系统。因为当前程序不退出，下一个程序就没有机会运行。

## 3. 用户程序如何进入内核

用户程序运行在 U-mode，内核运行在 S-mode。用户程序不能直接调用内核函数，否则权限隔离就没有意义了。

所以用户程序要请求内核服务时，需要执行：

```text
ecall
```

`ecall` 会让 CPU 从 U-mode 陷入 S-mode。内核再根据寄存器里的 syscall id 判断用户想做什么。

一个 `println!` 的链路可以这样讲：

```text
println!
  -> user console
  -> sys_write
  -> syscall 函数
  -> ecall 指令
  -> 内核 handle_syscall
  -> console_putchar
```

所以 `syscall` 是软件封装，`ecall` 是硬件指令。

## 4. 用户程序为什么要提前打包

第二章还没有文件系统，内核启动后不能去硬盘里找用户程序。因此用户程序必须在构建阶段就被塞进内核镜像。

构建流程是：

```text
编译用户程序
  -> build.rs 收集用户程序
  -> 生成 app.asm
  -> app.asm 用 .incbin 嵌入用户程序二进制
  -> 内核链接时把这些 app 一起带上
```

运行时，内核通过 `AppMeta::locate()` 找到这些 app 的边界，再把当前 app 复制到指定运行地址。

## 5. ch2-moving-tangram 的设计思路

老师给的进阶任务是通过多程序/多批次方式，逐块渲染七巧板组成的 “O/S” 图案。

我当前实现的思路是：

```text
内核仍然按 ch2 批处理方式运行 app0-app7
  -> 每完成一个 app，completed_apps += 1
  -> 所有 app 跑完后，进入 graphics::demo
  -> graphics 根据批处理完成节奏逐块画出 O/S 图案
```

这不是为了单纯炫技画图，而是把“批处理的一步一步推进”变成可见的图形变化。

如果讲给别人听，我会说：

```text
ch2 的抽象是“一批程序顺序完成”。
moving tangram 把这个抽象映射成“一块一块拼出 OS 图案”。
```

## 6. 图形输出怎么做

图形输出不是 `println!`。`println!` 是字符输出，走 SBI console。图形输出要写 framebuffer。

我这里使用 QEMU 提供的 VirtIO-GPU：

```text
QEMU virtio-gpu-device
  -> virtio-drivers
  -> setup_framebuffer
  -> 写像素
  -> gpu.flush
  -> 图形窗口显示
```

`graphics.rs` 里做了几件事：

```text
VirtioHal：提供 DMA 内存。
FramebufferCanvas：把像素写进 framebuffer。
draw_polygon：填充三角形/四边形。
piece(index)：定义每一块七巧板。
demo：逐块绘制并 flush。
```

## 7. 调试过程中最有价值的三个坑

第一个坑：缺 `rust-objcopy`。

ch2 构建用户程序镜像时需要裁剪 ELF，因此要安装 `cargo-binutils` 和 `llvm-tools-preview`。

第二个坑：Rust 2024 的 unsafe 规则。

`asm!("ecall")` 是裸机系统调用汇编。即使它在 `unsafe fn` 里面，也需要显式：

```rust
unsafe {
    asm!("ecall", ...);
}
```

第三个坑：地址冲突。

加入图形模块后，内核变大。原本用户程序加载到 `0x8040_0000`，会和内核区域冲突。现象是：

```text
QEMU 启动了
日志停在 app meta ready
没有 load app0
图形窗口 inactive
```

把 ch2 用户程序基址改成：

```text
0x8100_0000
```

之后，app0-app7 能正常跑完，并进入 VirtIO-GPU 初始化。

## 8. 当前进阶测试状态

我现在完成的是进阶图形 demo 的运行验证：

```text
cargo build 通过
QEMU 能启动
app0-app7 顺序运行
进入 VirtIO-GPU 初始化
图形窗口显示 O/S 七巧板
右侧 S 越界裁剪问题已修正
```

但是要注意，当前版本为了方便观察图形，最后不会自动关机，而是停在 `spin_loop` 保持窗口。

所以：

```text
它适合人工观察进阶图形 demo。
它不适合直接跑 test.sh 自动 checker。
```

如果后面要提交一个同时支持 checker 和 demo 的版本，最好加 feature 开关：

```text
demo 模式：保留窗口。
test 模式：跑完后 shutdown(false)。
```

## 9. 我会怎样总结第二章

第二章让我第一次看到“操作系统运行用户程序”的闭环：

```text
构建阶段把 app 放进内核
运行阶段内核找到 app
内核准备用户态上下文
CPU 进入用户态执行 app
app 通过 ecall 回内核
内核处理 syscall
app exit 后运行下一个 app
```

moving-tangram 则把这个闭环变成了可视化实验。它让我意识到，OS 不是只背概念，很多问题最后都会落到地址、链接、寄存器、设备和调试输出上。

