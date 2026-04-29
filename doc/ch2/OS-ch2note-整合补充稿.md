# OS ch2 整合补充稿：批处理、syscall、图形扩展与 AI 协作

> 本文是第二章阶段性整合稿，后续可以继续和课堂笔记、Guide/Book 内容合并。

## 一、第二章的主线

第二章的目标是实现一个最小批处理系统。它要解决的问题是：内核如何顺序运行多个用户程序。

这一章的主线不是图形，也不是多任务，而是：

```text
用户程序如何被打包
用户程序如何被定位
用户程序如何被加载到内存
CPU 如何进入用户态
用户程序如何通过 ecall 回到内核
内核如何处理 write / exit
内核如何继续运行下一个用户程序
```

这构成了一个最小的“应用执行环境”。

## 二、从构建到运行的完整闭环

第二章分成两个阶段。

构建阶段：

```text
用户程序源码
  -> 编译成 RISC-V ELF
  -> rust-objcopy 裁剪成二进制
  -> build.rs 生成 app.asm
  -> app.asm 被链接进内核
```

运行阶段：

```text
内核启动
  -> 初始化 BSS / console / syscall
  -> AppMeta::locate 找到用户程序元信息
  -> 把当前 app 复制到运行基址
  -> LocalContext::user 创建用户态上下文
  -> ctx.execute 进入 U-mode
  -> 用户程序 ecall
  -> 内核 handle_syscall
  -> exit 后进入下一个 app
```

这让我理解到：ch2 的 app 不是运行时从磁盘读出来的，而是在内核镜像里“内置携带”的。

## 三、syscall 的本质

syscall 的本质是受控跨特权级调用。

用户态不能直接调用内核函数，所以需要：

```text
a7 放 syscall id
a0-a5 放参数
执行 ecall
```

CPU 进入内核态后，内核根据 `scause` 判断 trap 原因，再根据 `a7` 分发到具体 syscall。

我现在可以这样区分：

```text
syscall：软件层面的封装和约定。
ecall：硬件层面的陷入指令。
trap：从用户态进入内核态的控制流事件。
```

## 四、moving-tangram 扩展实验

进阶任务要求基于 ch2 的多程序/多批次方式，逐块渲染七巧板 “O/S” 图案。

当前实现采用以下思路：

```text
保留 ch2 原有批处理流程
  -> 每完成一个 app，completed_apps += 1
  -> 所有 app 跑完后调用 graphics::demo(completed_apps)
  -> 图形模块逐块绘制 O/S 七巧板
```

它的意义不是“把 ch2 变成图形系统”，而是把批处理过程可视化。

```text
app 顺序完成的过程
  -> 映射成图形逐块出现的过程
```

## 五、图形实现结构

新增模块：

```text
src/graphics.rs
```

它包含：

```text
VirtioHal
  负责给 virtio-drivers 提供 DMA 分配。

FramebufferCanvas
  负责向 framebuffer 写像素。

draw_polygon
  负责填充三角形和四边形。

piece(index)
  定义 O/S 七巧板每一块。

demo(completed_apps)
  初始化 VirtIO-GPU，并逐块 flush 到窗口。
```

QEMU runner 改为：

```text
qemu-system-riscv64
  -machine virt
  -display gtk
  -serial stdio
  -device virtio-gpu-device
  -bios none
  -kernel ...
```

为了避免本地 PATH 问题，runner 使用了 QEMU 的绝对路径。

## 六、调试记录

### 1. 缺 cargo clone / TG_USER_DIR

ch2 的 `build.rs` 默认可能尝试 `cargo clone` 用户程序。为了避免依赖额外命令，配置改成使用仓库中已有的：

```text
../tg-rcore-tutorial-user
```

### 2. 缺 rust-objcopy

`build.rs` 需要 `rust-objcopy` 裁剪用户程序镜像，因此补充安装：

```text
cargo-binutils
llvm-tools-preview
```

### 3. Rust 2024 unsafe asm

`tg-rcore-tutorial-syscall/src/user.rs` 中的 `asm!("ecall")` 在 Rust 2024 下需要显式 unsafe 块。

### 4. framebuffer 借用问题

VirtIO-GPU 的 framebuffer 生命周期依赖 gpu，不能简单把两者长期存在同一个结构体里。最终采用短作用域临时借用 framebuffer，绘制一块后释放借用，再调用 `gpu.flush()`。

### 5. app 基址冲突

加入图形模块后内核变大，原本的：

```text
0x8040_0000
```

不再安全。修正为：

```text
0x8100_0000
```

之后能完整跑完 app0-app7。

### 6. S 图案右侧被裁剪

QEMU 返回实际显示分辨率为 `640x480`，而原图形坐标按 `800` 宽度设计，导致 S 右侧被裁掉。修正方式是把 S 的 x 坐标压回 `410..630` 范围。

## 七、当前测试与验证状态

已完成：

```text
cargo build 通过
QEMU runner 能启动
批处理 app0-app7 能运行
进入 VirtIO-GPU 初始化
图形窗口能显示 O/S 七巧板
S 右侧裁剪问题已修复
```

演示动图：

```text
doc/ch2/ch2-moving-tangram-demo.gif
```

关于 `test.sh`：

```text
当前版本是进阶图形 demo，最后 spin_loop 保持窗口。
因此它不会自动退出，不适合直接用 test.sh 做自动 checker。
这不是基础功能失败，而是 demo 模式和自动测试模式目标不同。
```

后续建议：

```text
增加 feature 或配置开关：
  demo：保留窗口，适合展示。
  test：跑完后 shutdown(false)，适合 checker。
```

## 八、AI 协作总结

这次 AI 协作不是简单“让 AI 写代码”，而是用 AI 做三个层面的辅助：

```text
工程实现：补 VirtIO-GPU、QEMU runner、framebuffer 绘制。
报错定位：处理 cargo clone、rust-objcopy、unsafe asm、借用检查。
原理解释：把批处理、syscall、地址冲突和图形设备联系起来。
```

我的收获是：以后指挥 AI 时，不能只说“帮我改好”，而要逐步问：

```text
现在卡在构建期还是运行期？
是工具链问题、Rust 类型问题，还是 OS 内存布局问题？
能不能通过串口日志证明执行到了哪一步？
这个修改对原本 OS 结构有没有副作用？
```

这正好符合课程要求：通过 AI 学 OS，而不是绕过 OS。
