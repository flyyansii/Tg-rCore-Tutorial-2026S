# rCore ch6 学习复盘与问答底稿

## 复盘目标

这一份文档整理 ch6 学习时最容易混淆的问题，尤其是：

- 文件描述符到底是什么。
- UserBuffer 为什么必须翻译。
- easy-fs 如何把文件名映射到磁盘块。
- link/unlink 和普通 read/write 有什么不同。
- fstat 为什么要写回用户地址。
- ch6-breakout 为什么正好体现文件系统保存/恢复。

## Q1：ch6 和 ch5 的主要区别是什么？

我的初始理解：

> ch5 管进程，ch6 管文件。

修正后的答案：

这个方向是对的，但更完整的说法是：ch5 让“运行实体”可管理，ch6 让“持久数据”可管理。

ch5 解决：

```text
谁在运行？
谁是父进程？
谁退出了？
谁等待谁？
谁获得 CPU？
```

ch6 解决：

```text
数据放在哪里？
如何按名字找到数据？
如何让进程通过 fd 操作数据？
如何把数据同步到块设备？
如何保存到 fs.img？
```

## Q2：文件描述符 fd 是文件本身吗？

不是。

fd 是当前进程 fd 表中的下标。真正的文件对象在内核里。

```text
fd
-> current.fd_table[fd]
-> OSInode / Console / 特殊设备
-> Inode
-> easy-fs
```

所以用户态只拿到一个整数。这样做的好处是：

- 用户不能直接改内核对象。
- 内核可以检查权限。
- 不同进程可以有各自 fd 表。
- 同一接口可以表示不同 I/O 对象。

## Q3：为什么 fd 不是全局的？

因为 fd 属于进程。

例如：

```text
进程 A: fd 3 -> filea
进程 B: fd 3 -> fileb
```

两个 `fd=3` 并不冲突，因为它们查的是不同进程的 `fd_table`。

这就像每个班级都有“3号同学”，但要先说明是哪个班。

## Q4：UserBuffer 为什么要翻译？

因为用户传来的 `buf` 是用户虚拟地址。

内核不能直接用：

```rust
buf as *mut u8
```

它必须通过当前进程地址空间翻译：

```text
用户虚拟地址
-> 页表
-> 物理页
-> 内核可访问地址
```

如果不翻译，会出现：

- 读错地址。
- 写错内存。
- 破坏内核。
- 绕过权限检查。

## Q5：read/write 为什么要先找当前进程？

因为 fd_table 和用户页表都属于当前进程。

`write(fd, buf, len)` 需要两个上下文：

```text
当前进程 fd_table：决定 fd 指向谁。
当前进程 address_space：决定 buf 怎么翻译。
```

所以系统调用实现里第一步常常是：

```rust
let current = PROCESSOR.get_mut().current().unwrap();
```

## Q6：open 返回的 fd 是怎么来的？

open 会：

1. 从用户空间读取 path。
2. 调用文件系统打开或创建文件。
3. 得到一个内核文件对象。
4. 找当前进程 fd_table 中空位。
5. 把对象放进去。
6. 返回这个空位下标。

所以 open 的返回值不是文件内容，而是之后访问这个文件的句柄编号。

## Q7：easy-fs 为什么要有 inode？

文件名不适合直接管理数据块。

文件名可以变，可以有多个名字指向同一个文件。真正稳定的对象是 inode。

关系是：

```text
目录项: 文件名 -> inode 编号
inode: 文件大小/类型/数据块指针
数据块: 真实文件内容
```

这就是为什么 link/unlink 操作目录项时，不能简单删除数据块。

## Q8：linkat 是复制文件吗？

不是。

`linkat(old, new)` 是创建硬链接，让 `new` 指向 `old` 的同一个 inode。

```text
old -> inode 7
new -> inode 7
```

文件内容没有复制，只是多了一个名字。

## Q9：unlinkat 是删除文件内容吗？

不一定。

`unlinkat(path)` 删除的是目录项。

如果删除后 inode 的 `nlink` 仍然大于 0，文件内容还在。

只有当：

```text
nlink == 0
```

才释放数据块和 inode。

## Q10：为什么要给 DiskInode 加 nlink？

因为硬链接需要知道有多少个目录项指向同一个 inode。

没有 `nlink`，内核无法判断：

- unlink 后是否还能通过别的名字访问文件。
- 什么时候可以真正释放数据块。

所以本次修改在 `DiskInode` 中加入了硬链接计数。

## Q11：fstat 和 read 有什么区别？

`read` 读的是文件内容。

`fstat` 查的是文件元信息。

例如：

```text
read -> "Hello, world!"
fstat -> inode 编号、文件类型、硬链接数
```

fstat 的结果写入用户传来的 `Stat` 结构体。

## Q12：为什么 fstat 也需要地址翻译？

因为 `st` 指针来自用户程序。

内核要把结果写回用户空间，所以要确认：

- 这个地址在当前进程中存在。
- 它有写权限。
- 写入不会越权。

所以仍然需要 `address_space.translate`。

## Q13：block cache 是不是多此一举？

不是。

块设备按块访问，一块通常 512 字节或 4096 字节。文件系统结构也往往跨块存储。

block cache 的作用是：

- 缓存磁盘块。
- 让文件系统能像修改内存一样修改块内容。
- 最后统一同步到块设备。

没有 block cache，每次读写都直接访问设备，代码复杂且性能差。

## Q14：fs.img 到底是什么？

`fs.img` 是 QEMU 挂载的磁盘镜像文件。

在宿主机上它只是一个普通文件，但在 QEMU 里的内核看来，它是一个块设备。

```text
Windows 文件 fs.img
-> QEMU virtio-blk
-> rCore block driver
-> easy-fs
```

所以 ch6 文件写入最终会改变这个镜像里的内容。

## Q15：为什么说 ch6 串起了 ch4 和 ch5？

因为文件系统调用同时依赖：

- ch4 的地址空间翻译。
- ch5 的当前进程和 fd_table。
- ch6 的文件系统对象。

没有 ch4，内核不知道怎么访问用户 buffer。

没有 ch5，内核不知道 fd 属于哪个进程。

没有 ch6，内核没有持久化文件对象。

## Q16：为什么 breakout 用 fd=3 输出图形？

这是为了体现统一 I/O 抽象。

用户程序调用：

```rust
write(3, frame_bytes)
```

内核判断 `fd == GRAPHICS_FD`，就不走普通文件，而是把这段 bytes 当作一帧游戏画面交给 GPU 渲染。

这说明 fd 不一定对应磁盘文件，也可以对应设备。

## Q17：为什么键盘输入返回 -2？

breakout 游戏循环不能因为没按键就卡死。

如果 `read(stdin)` 在没有键盘事件时阻塞，游戏画面就不动了。

所以内核在没有键时返回 `-2`，用户态 `try_getchar` 可以理解为“当前没有输入”，然后继续更新小球。

## Q18：breakout 的保存/恢复如何体现文件系统？

保存：

```text
按 S
-> open("breakout.sav", CREATE | WRONLY)
-> write SaveData
-> close
```

恢复：

```text
按 R
-> open("breakout.sav", RDONLY)
-> read SaveData
-> 检查 magic
-> 恢复状态
-> close
```

这里完整用到了 ch6 的 open/read/write/close。

## Q19：为什么要映射更多 MMIO？

原 ch6 只需要块设备。

breakout 又需要 GPU 和键盘。

所以 MMIO 需要覆盖：

```text
virtio-blk      0x1000_1000
virtio-gpu      0x1000_2000
virtio-keyboard 0x1000_3000
```

如果不映射，内核访问设备寄存器会失败。

## Q20：ch6 测试中 panic 是否等于失败？

不一定。

有些用户测试故意触发 page fault 或断言，用来验证异常路径。判断测试是否通过要看最后输出：

```text
ch6 Usertests passed!
Basic usertests passed!
```

本次这两条都已经验证过。

## Q21：本章我最容易讲错的地方是什么？

最容易讲错的是：

```text
unlink = 删除文件
```

更准确是：

```text
unlink = 删除一个目录项/名字
```

只有当 inode 没有任何硬链接时，才删除真实数据。

## Q22：如果让我向别人解释 ch6，我会怎么说？

我会这样讲：

ch6 让用户程序可以通过文件描述符访问文件。用户态只看到 fd 和 buffer，但内核会根据当前进程找到 fd_table，再根据当前页表翻译用户 buffer，然后通过 OSInode、Inode、block cache、virtio-blk 把数据读写到 fs.img。

这章的重点不是某个 API，而是这条完整链路。

## Q23：本次 AI 协作学到了什么？

这次不是只让 AI 写代码，而是反复追问：

- fd 到底是不是文件。
- UserBuffer 为什么不能直接用。
- easy-fs 的 inode 和文件名有什么关系。
- link/unlink 为什么要改底层 nlink。
- breakout 为什么要用文件保存进度。

这样做的好处是：代码不是黑箱，而是能把 Guide 的概念和仓库里的模块对应起来。

## Q24：后续可以继续改进什么？

后续可以继续改进：

- 把 `test.sh` 的乱码注释修成 UTF-8。
- 给 breakout 增加更清晰的 UI 提示。
- 把特殊图形 fd 抽象成更正式的设备文件。
- 在文档里补充更多 easy-fs 磁盘布局图。
- 录制 breakout gif 作为 demo。

