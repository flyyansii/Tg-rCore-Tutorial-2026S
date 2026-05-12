# OS ch8 线程、同步与死锁检测整合补充稿

## 一、本章总览

第八章是在前面进程、文件系统、管道、信号等机制基础上，引入线程和同步原语。它解决的问题是：同一个进程内部如何拥有多条并发执行流，以及这些执行流如何安全地访问共享资源。

本章的核心变化可以概括为：

```text
从“进程是调度单位”
变成“线程是调度单位，进程是资源容器”。
```

对应代码中：

```text
Process：保存资源。
Thread：保存执行上下文。
PThreadManager：同时管理进程和线程。
```

## 二、Process 和 Thread 的分工

ch8 中的 `Process` 包含：

```text
pid
address_space
fd_table
signal
semaphore_list
mutex_list
condvar_list
```

这些都是同一进程内线程共享的资源。

`Thread` 包含：

```text
tid
ForeignContext
```

线程保存的是“执行到哪里”的信息，例如寄存器、PC、SP 和 satp。因为同一进程的线程共享地址空间，所以多个线程可以访问同一份堆数据和全局变量。

## 三、为什么线程需要同步

线程共享地址空间带来方便，也带来竞态。

例如多个线程同时修改一个全局变量：

```text
Thread A 读取 counter = 0
Thread B 读取 counter = 0
Thread A 写回 counter = 1
Thread B 写回 counter = 1
```

虽然两个线程都执行了加一，但最终结果只加了一次。这就是竞态条件。

因此 ch8 引入：

- `Mutex`：保护临界区。
- `Semaphore`：管理计数型资源。
- `Condvar`：等待某个条件成立。

## 四、线程创建流程

用户调用：

```rust
thread_create(entry, arg)
```

内核流程：

```text
进入系统调用
找到当前进程
在当前进程地址空间中分配用户栈
构造用户态上下文
设置入口地址 entry
设置参数 a0 = arg
创建 Thread
加入 PThreadManager
返回 tid
```

注意：新线程没有新建进程，也没有复制地址空间，它和当前线程属于同一个进程。

## 五、线程等待流程

用户调用：

```rust
waittid(tid)
```

内核需要判断：

- 不能等待自己。
- 如果目标线程已经退出，返回退出码。
- 如果目标线程还没退出，则返回等待失败或按管理器策略处理。

`waittid` 是线程粒度的回收机制，对应进程粒度的 `waitpid`。

## 六、互斥锁机制

互斥锁的作用是保证同一时刻只有一个线程进入临界区。

流程：

```text
mutex_create
  -> 在 Process.mutex_list 中创建锁

mutex_lock
  -> 如果锁空闲，当前 tid 持有锁
  -> 如果锁被占用，当前线程阻塞

mutex_unlock
  -> 释放锁
  -> 如果有等待线程，唤醒一个
```

ch8 中锁属于进程资源，所以同一个进程内所有线程共享同一个 `mutex_list`。

## 七、信号量机制

信号量适合表示多份资源。

流程：

```text
semaphore_create(n)
  -> 创建初始计数为 n 的资源

semaphore_down
  -> 如果 count > 0，则 count -= 1
  -> 否则当前线程阻塞

semaphore_up
  -> count += 1 或唤醒等待线程
```

mutex 可以看成特殊的 0/1 资源；semaphore 更一般，可以表示多个相同资源。

## 八、条件变量机制

条件变量用于等待条件，而不是直接保护资源。

典型语义：

```text
线程持有 mutex
发现条件不满足
condvar_wait 释放 mutex 并阻塞
其他线程修改条件后 condvar_signal
等待线程被唤醒
线程重新竞争 mutex
继续检查条件
```

条件变量必须和 mutex 一起理解，否则容易漏掉“等待时释放锁”这一关键点。

## 九、阻塞与唤醒

ch8 主调度循环中新增了对阻塞的处理。

当这些系统调用返回 `-1`：

```text
SEMAPHORE_DOWN
MUTEX_LOCK
CONDVAR_WAIT
```

内核会调用：

```text
make_current_blocked()
```

当前线程从 ready queue 中移除，直到资源释放或条件满足。

当释放资源时：

```text
semaphore_up
mutex_unlock
condvar_signal
```

可能返回一个等待线程的 TID，内核调用：

```text
re_enque(tid)
```

把它重新放回就绪队列。

## 十、死锁检测

死锁是多个线程互相等待资源，形成闭环。

例子：

```text
Thread 1 持有 Mutex A，等待 Mutex B。
Thread 2 持有 Mutex B，等待 Mutex A。
```

ch8 exercise 要求新增系统调用：

```text
enable_deadlock_detect
syscall ID = 469
```

开启后，`mutex_lock` 和 `semaphore_down` 在阻塞前要判断是否会导致死锁。如果会导致死锁，应拒绝操作并返回：

```text
-0xDEAD
```

## 十一、死锁检测算法

题目提供的是安全性检测算法，核心数据结构：

```text
Available：每类资源剩余数量。
Allocation：每个线程已分配资源数量。
Need：每个线程还需要的资源数量。
Work：模拟当前可用资源。
Finish：模拟线程是否可以完成。
```

检测流程：

```text
Work = Available
Finish 全部设为 false
寻找 Need[i] <= Work 的线程 i
如果找到，假设它运行完成并释放资源
Work += Allocation[i]
Finish[i] = true
重复寻找
如果所有 Finish 都为 true，系统安全
否则系统不安全，存在死锁风险
```

通俗理解：

```text
先假装未来继续运行。
如果能排出一个所有线程都能完成的顺序，就安全。
如果排不出来，就说明可能卡死。
```

## 十二、ch8_usertest

`ch8_usertest` 是本章综合测试入口。它会依次 fork 子进程并 exec 多个测试程序：

```text
threads
threads_arg
mpsc_sem
sync_sem
race_adder_mutex_blocking
phil_din_mutex
test_condvar
pipetest
ch8_deadlock_mutex1
ch8_deadlock_sem1
ch8_deadlock_sem2
```

其中：

- `ch8_deadlock_mutex1` 测试同一线程重复锁同一把阻塞 mutex。
- `ch8_deadlock_sem1` 构造信号量资源等待环，要求检测到死锁。
- `ch8_deadlock_sem2` 构造安全场景，要求不能误报死锁。

## 十三、学习重点

本章最重要的是分清几个层次：

```text
Process：资源归属层。
Thread：执行调度层。
Sync primitives：线程协作层。
Deadlock detection：资源申请安全性检查层。
```

不要把线程简单理解为“更小的进程”。线程和进程最大的区别是共享资源。正因为共享资源，所以需要同步；正因为同步会等待资源，所以可能死锁；正因为可能死锁，所以 exercise 要做死锁检测。

## 十四、AI 协作与 GitHub 交付

本章文档由我和 AI 协作整理，内容包括：

- ch8 概念复盘。
- Process/Thread 分离解释。
- 线程创建、等待、同步原语调用链。
- 死锁检测算法说明。
- exercise 测试用例分析。

整理后的四份 Markdown 文件统一放在：

```text
doc/ch8/
```

并提交到 GitHub 仓库，用于满足课程要求中的 Markdown 文档、AI 协作记录和持续迭代提交要求。
