# rCore ch7 管道、重定向与 Pacman 整合补充稿

## 1. 这一章到底在解决什么问题

ch7 的主题是进程间通信和更完整的 Unix 风格 I/O。ch6 已经让进程能通过文件系统保存数据，但进程之间如果想边运行边传数据，还需要一种更轻量的通道。

ch7 引入了管道 pipe。管道可以理解为内核里的一段环形缓冲区，一端写，一端读。

一句话概括：

```text
ch6: 数据可以保存到文件。
ch7: 数据可以在进程之间流动。
```

## 2. 从文件系统到管道：为什么 fd 要统一抽象

ch6 的 fd 多数指向普通文件。ch7 里 fd 不再只表示文件，还可以表示：

- 普通文件。
- 管道读端。
- 管道写端。
- 标准输入。
- 标准输出。
- 本实验扩展的图形输出 fd。

所以 ch7 把 fd table 的元素抽象成 `Fd` 枚举：

```text
Fd::File
Fd::PipeRead
Fd::PipeWrite
Fd::Empty
```

这样 `read/write` 就能先查 fd，再根据 fd 的真实类型分发。

## 3. pipe 的直觉

管道是一个“内核队列”：

```text
writer -> [ ring buffer in kernel ] -> reader
```

用户程序看不到这段缓冲区，只能拿到两个 fd：

```text
pipe_fd[0] = read end
pipe_fd[1] = write end
```

写端写入数据，读端读出数据。

## 4. 为什么 pipe 通常配合 fork 使用

典型流程是：

```text
父进程 pipe()
-> 得到 read_fd/write_fd
-> fork()
-> 子进程继承 fd_table
-> 父进程关闭读端，只写
-> 子进程关闭写端，只读
```

fork 后父子进程共享管道对象的引用，因此父进程写进去的数据，子进程能读出来。

## 5. ring buffer 其实就是循环队列

我们之前讨论过：ring buffer 听起来高级，本质上就是数据结构里的循环队列。

它通常维护：

```text
buffer: [u8; N]
head: 读位置
tail: 写位置
status: empty/full/normal
```

写数据：

```text
buffer[tail] = byte
tail = (tail + 1) % N
```

读数据：

```text
byte = buffer[head]
head = (head + 1) % N
```

用取模实现尾巴绕回开头，所以叫 ring。

## 6. pipe 的阻塞语义

教学系统里管道可能返回特殊值，例如：

```text
-2 表示暂时不能读/写
```

用户库可以在 `pipe_read/pipe_write` 中循环：

```text
如果返回 -2
    sched_yield()
    过一会儿重试
```

这体现了 ch3/ch5 的调度能力：I/O 暂时不可用时，进程主动让出 CPU。

## 7. 重定向是什么

重定向的本质是改变 fd 指向。

例如 shell 中：

```text
cat file > out
```

直觉上是“把输出写到 out 文件”，内核视角其实是：

```text
stdout(fd=1)
原本指向控制台
现在改成指向 out 文件
```

之后程序照常 `write(1, ...)`，但 fd=1 背后的对象已经变了。

## 8. dup 的意义

`dup` 或类似机制可以复制 fd，使两个 fd 指向同一个内核对象。

```text
fd 1 -> stdout
fd 4 -> same stdout object
```

重定向常常需要：

1. 打开目标文件。
2. 关闭原 stdout。
3. 复制目标文件 fd 到 stdout 位置。
4. exec 目标程序。

程序本身不用知道 stdout 被换掉了。

## 9. exec 后 fd 为什么还在

`exec` 替换当前进程的程序代码和地址空间，但通常保留进程身份和 fd_table。

这就是 shell 能做重定向的原因：

```text
shell 子进程先改 fd_table
-> exec 成目标程序
-> 目标程序继承改过的 fd_table
```

如果 exec 把 fd_table 也清空，重定向就失效了。

## 10. 命令行参数如何传递

ch7 也涉及命令行参数。exec 时，内核不仅要加载新 ELF，还要把参数放到用户栈上。

大致流程：

```text
用户传入 path 和 args
-> 内核读取用户字符串数组
-> 创建/替换地址空间
-> 在用户栈上压入参数字符串
-> 设置 argc/argv
-> 设置用户上下文
-> 返回用户态从新程序入口开始
```

这和 C 语言的：

```c
int main(int argc, char **argv)
```

是同一个思想。

## 11. 本仓库 ch7 的组件化差异

Guide 常见结构：

```text
os/src/fs/pipe.rs
os/src/syscall/fs.rs
os/src/task/process.rs
```

本仓库结构：

```text
tg-rcore-tutorial-ch7/src/fs.rs
-> Fd 枚举、read/write 统一接口、FS

tg-rcore-tutorial-ch7/src/main.rs
-> IO syscall trait impl，包括 pipe/read/write/open/close

tg-rcore-tutorial-ch7/src/process.rs
-> Process，fd_table: Vec<Option<Mutex<Fd>>>

tg-rcore-tutorial-easy-fs
-> PipeReader/PipeWriter/UserBuffer 实现
```

所以读代码时要按功能找，而不是死找 Guide 的文件名。

## 12. ch7 启动流程

1. QEMU 启动内核。
2. 挂载 `fs.img`。
3. 内核初始化日志、堆、页表。
4. 映射 virtio-blk MMIO。
5. 本实验扩展映射 virtio-gpu 和 virtio-keyboard。
6. 初始化 syscall。
7. 初始化 signal syscall。
8. 从文件系统打开 `initproc`。
9. 创建 initproc 进程。
10. initproc 根据 `CHAPTER` 选择目标程序。
11. 测试模式 `CHAPTER=-7` 执行 `ch7b_usertest`。
12. 默认游戏模式 `CHAPTER=pacman` 执行 `ch7_pacman`。

## 13. pipe 系统调用流程

```text
用户 pipe(pipe_fd_ptr)
-> ecall
-> IO::pipe
-> make_pipe()
-> 得到 PipeReader/PipeWriter
-> 分配 read_fd/write_fd
-> 写回用户 pipe_fd[0], pipe_fd[1]
-> fd_table push Fd::PipeRead / Fd::PipeWrite
```

关键是写回用户数组时仍然需要地址翻译，因为 `pipe_fd_ptr` 是用户虚拟地址。

## 14. pipe read/write 流程

写：

```text
write(write_fd, buf)
-> fd_table[write_fd]
-> Fd::PipeWrite
-> PipeWriter::write
-> 写入 ring buffer
```

读：

```text
read(read_fd, buf)
-> fd_table[read_fd]
-> Fd::PipeRead
-> PipeReader::read
-> 从 ring buffer 取出
```

如果缓冲区空或满，就可能返回 `-2`，用户库让出 CPU 后重试。

## 15. ch7 Pacman 扩展

本次扩展实现了用户态简化 Pacman：

```text
tg-rcore-tutorial-user/src/bin/ch7_pacman.rs
```

基本功能：

- WASD/方向键移动。
- 地图墙体。
- 豆子收集。
- 分数。
- 生命值。
- 幽灵追踪。
- 胜利/失败状态。

内核扩展：

```text
tg-rcore-tutorial-ch7/src/graphics.rs
tg-rcore-tutorial-ch7/src/keyboard.rs
```

## 16. Pacman 图形输出链

```text
用户态 Game::submit
-> 构造 PacmanFrame
-> write(fd=3, frame)
-> IO::write 识别 GRAPHICS_FD
-> graphics::submit_pacman_frame
-> VirtIO-GPU framebuffer 绘制地图、豆子、Pacman、幽灵
-> flush
-> QEMU GTK 窗口显示
```

## 17. Pacman 键盘输入链

```text
用户态 try_getchar()
-> read(STDIN)
-> IO::read
-> keyboard::take()
-> VirtIOInput pop_pending_event
-> keycode 转 WASD
-> 写回用户 buffer
```

没有按键时返回 `-2`，游戏继续刷新，不会阻塞卡死。

## 18. 测试隔离

默认 `cargo run` 使用 GTK + GPU + keyboard，跑 Pacman。

测试脚本中强制设置 headless runner：

```text
qemu-system-riscv64 -machine virt -nographic ...
```

这样 CI 或 `./test.sh` 不会打开图形窗口，也不会卡在游戏交互里。

## 19. 验证结果

已验证：

```text
cargo build
-> passed

CHAPTER=-7 cargo run
-> Basic usertests passed!
```

默认 `cargo run` 会启动 Pacman 游戏窗口，需要手动关闭或按 `Q` 退出。

## 20. 本章最重要的理解

ch7 的核心不是“多了一个 pipe 函数”，而是 fd 抽象进一步统一了：

```text
同一个 read/write
可以操作普通文件
可以操作管道
可以操作标准输入输出
也可以扩展到图形设备
```

这就是 Unix 风格 I/O 的力量。
## GTK QEMU 图形 Demo 补充记录

本章在管道、重定向、参数传递和信号机制的学习基础上，额外做了一个 `ch7-pacman` 图形展示路径。默认 `cargo run` 会打开 QEMU GTK 窗口，由内核侧的固定脚本驱动 Pacman 和幽灵移动，并把每一帧写入 VirtIO-GPU framebuffer。

演示图如下：

![ch7-pacman-demo](../assets/ch7-pacman-demo.gif)

这条 demo 路径和原教程测试路径保持隔离：`cargo run` 默认用于图形展示；`test.sh base` 会设置 `CHAPTER=-7` 并使用 headless runner，因此仍然执行原来的 ch7 基础测试，不依赖 GTK 窗口。

这次调试里最容易混淆的地方是：Pacman demo 看起来像“应用程序画图”，但为了稳定展示和录制，我采用的是内核固定脚本图形路径。它仍然能体现本章之后内核拥有的设备抽象能力：QEMU 提供 VirtIO-GPU，内核通过 MMIO 初始化设备，绘制 framebuffer，再 flush 到 GTK 窗口。对应验证日志为：

```text
[ch7-pacman] virtio-gpu ready
[ch7-pacman] first frame visible
[ch7-pacman] Test ch7 pacman OK!
```
