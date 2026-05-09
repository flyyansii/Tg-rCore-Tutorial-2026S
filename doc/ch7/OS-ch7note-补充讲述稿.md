# rCore ch7 补充讲述稿：管道就是进程之间的“传送带”

## 开场

ch6 让程序能把数据写进文件，像是给程序配了硬盘。ch7 让程序之间能边运行边传数据，像是在两个程序之间架了一条传送带。

这条传送带就是 pipe。

## pipe 的通俗理解

假设有两个进程：

```text
进程 A 生产数据
进程 B 消费数据
```

如果用文件通信，A 要先写文件，B 再读文件。这个方式可以，但不够轻。

管道更像：

```text
A 写入一端
B 从另一端读出
```

数据暂时存在内核的环形缓冲区里。

## ring buffer 不神秘

ring buffer 其实就是循环队列。

它比普通数组多两个指针：

```text
head: 下一次读的位置
tail: 下一次写的位置
```

读写到数组末尾后，通过取模回到开头。

```text
tail = (tail + 1) % N
head = (head + 1) % N
```

所以它适合做流式数据缓冲。

## 为什么 pipe 要放在 fd_table 里

因为 Unix 希望所有 I/O 都尽量通过 fd 操作。

普通文件：

```text
write(file_fd, data)
```

管道：

```text
write(pipe_write_fd, data)
read(pipe_read_fd, data)
```

用户程序不需要知道背后是磁盘还是内存环形队列。它只看 fd 和 read/write。

## fork 后 fd 继承为什么重要

如果父进程 pipe 后 fork，子进程会继承父进程的 fd_table。

这意味着父子进程都知道同一条管道的读写端。

之后它们可以关闭自己不用的一端：

```text
父进程：关闭读端，只写。
子进程：关闭写端，只读。
```

这就形成了单向通信。

## 重定向的本质

重定向不是修改程序代码，而是修改 fd_table。

程序仍然写 stdout：

```text
write(1, data)
```

但 shell 在 exec 前把 fd=1 改成文件，于是输出自然进文件。

所以 shell 的能力来自：

- fork 保留 shell。
- 子进程改 fd。
- exec 目标程序。
- exec 后 fd_table 仍保留。

## ch7 Pacman 为什么合理

Pacman 本身不一定用 pipe，但它可以用 ch7 已经统一的 fd 机制。

我们把图形设备做成特殊 fd：

```text
fd=3
```

用户程序写 fd=3，内核就把数据解释成一帧画面。

键盘输入仍然走 stdin：

```text
read(0, buf)
```

这正好说明 ch7 的 fd 抽象已经足够统一。

## 本章一句话总结

ch7 让 fd 不再只是“打开的文件编号”，而是进程访问各种 I/O 对象的统一入口。管道、重定向、标准输入输出，甚至我们扩展的图形设备，都可以放进这套模型里理解。

