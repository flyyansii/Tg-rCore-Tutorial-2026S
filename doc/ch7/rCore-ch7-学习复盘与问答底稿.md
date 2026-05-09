# rCore ch7 学习复盘与问答底稿

## Q1：ch7 相比 ch6 新在哪里？

ch6 的重点是文件系统，数据可以保存到 `fs.img`。

ch7 的重点是管道，数据可以在进程之间流动。

```text
ch6: persistent storage
ch7: inter-process communication
```

## Q2：pipe 是文件吗？

pipe 不是磁盘文件，但它可以用 fd 表示。

这就是 Unix 的统一 I/O 抽象：

```text
read/write 可以操作普通文件
read/write 也可以操作 pipe
```

## Q3：ring buffer 是什么？

ring buffer 是循环队列。

它用固定数组加 head/tail 指针实现：

```text
写: tail = (tail + 1) % N
读: head = (head + 1) % N
```

优势是不用频繁移动数组内容。

## Q4：为什么 pipe 要返回两个 fd？

因为管道有两个端：

```text
pipe[0] = read end
pipe[1] = write end
```

读端只读，写端只写。

## Q5：为什么 pipe 常配合 fork？

因为 fork 会继承 fd_table。

父进程 pipe 后 fork，子进程也拥有同一条管道的读写端引用。之后父子进程关闭不用的一端，就能形成单向通信。

## Q6：pipe 空了怎么办？

如果读端发现缓冲区空，可能返回 `-2` 表示暂时没数据。

用户库可以：

```text
sched_yield()
稍后重试
```

这比死循环占满 CPU 更好。

## Q7：pipe 满了怎么办？

写端发现缓冲区满，也可以返回 `-2`。写进程让出 CPU，让读进程有机会读走数据。

这体现了进程调度和 IPC 的配合。

## Q8：重定向为什么不需要改程序代码？

因为程序只认 fd。

如果 shell 把 fd=1 从控制台换成文件，程序继续 `write(1, ...)`，输出自然进入文件。

## Q9：exec 后 fd_table 保留有什么意义？

shell 可以在子进程 exec 前改好 fd_table。

exec 后目标程序继承这个 fd_table，于是重定向、管道连接都仍然有效。

## Q10：ch7 的 `Fd` 枚举解决什么问题？

它让 fd_table 能同时存：

- 普通文件。
- 管道读端。
- 管道写端。
- 标准 IO 占位。

否则 fd_table 只能放 `FileHandle`，无法表示 pipe。

## Q11：Pacman 和 ch7 有什么关系？

Pacman 主要是扩展实验，用来验证内核可以支持用户态交互应用。

它利用了 ch7 的统一 fd 思想：

```text
stdin fd=0 -> keyboard input
graphics fd=3 -> framebuffer output
```

虽然游戏本身不依赖 pipe，但它体现了 fd 可以统一设备访问。

## Q12：为什么键盘 read 要非阻塞？

游戏循环不能卡在等待输入上。

没有按键时返回 `-2`，用户态继续更新幽灵和画面。

## Q13：为什么测试要 headless？

CI 或自动测试不能弹 GTK 窗口。

所以默认 `cargo run` 可以跑 Pacman，但 `test.sh` 用环境变量覆盖 runner，强制：

```text
-nographic
```

## Q14：本章最容易混淆的点是什么？

最容易把 pipe 当成磁盘文件。

更准确的理解是：

```text
pipe 是内核内存里的 IPC 对象
但它被包装成 fd
所以用户可以用 read/write 操作
```

## Q15：我应该如何向别人讲 ch7？

可以这样说：

ch7 把 fd 抽象推进了一步。fd 不只是文件下标，而是进程访问 I/O 对象的统一入口。管道把一个内核环形缓冲区拆成读端和写端，放进 fd_table。fork 后父子进程继承这些 fd，于是可以通过 read/write 进行进程间通信。重定向也是同一套思想：改 fd_table，而不是改程序代码。

