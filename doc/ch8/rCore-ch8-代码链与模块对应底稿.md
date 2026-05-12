# rCore ch8 代码链与模块对应底稿

## 目录结构

ch8 的代码树重点集中在内核线程模型和同步原语：

```text
tg-rcore-tutorial-ch8/
├── build.rs
├── exercise.md
├── src/
│   ├── main.rs
│   ├── process.rs
│   ├── processor.rs
│   ├── fs.rs
│   └── virtio_block.rs
tg-rcore-tutorial-user/
└── src/bin/
    ├── ch8_usertest.rs
    ├── ch8_deadlock_mutex1.rs
    ├── ch8_deadlock_sem1.rs
    ├── ch8_deadlock_sem2.rs
    ├── threads.rs
    ├── threads_arg.rs
    ├── mpsc_sem.rs
    ├── sync_sem.rs
    ├── race_adder_mutex_blocking.rs
    ├── phil_din_mutex.rs
    └── test_condvar.rs
```

模块职责：

- `main.rs`：内核启动、系统调用分发、主调度循环、同步阻塞处理。
- `process.rs`：定义 `Process` 和 `Thread`，处理 ELF 加载、fork、exec。
- `processor.rs`：定义全局 `PROCESSOR`，用 `PThreadManager` 同时管理进程和线程。
- `fs.rs`：文件系统和 fd 支持，沿用 ch6/ch7 能力。
- `virtio_block.rs`：块设备驱动，支持文件系统镜像。
- `tg_sync`：提供 `MutexBlocking`、`Semaphore`、`Condvar` 等同步原语实现。

## 总启动链

```mermaid
flowchart TD
    A["cargo run --features exercise"] --> B["build.rs 构建用户程序与 fs.img"]
    B --> C["QEMU 启动 ch8 内核"]
    C --> D["main.rs::rust_main()"]
    D --> E["zero_bss / console / heap"]
    E --> F["初始化 VirtIO block 和 easy-fs"]
    F --> G["建立 kernel_space 和 portal"]
    G --> H["注册 syscall: IO/process/scheduling/clock/signal/thread/sync_mutex"]
    H --> I["从 FS 读取 initproc"]
    I --> J["Process::from_elf(initproc)"]
    J --> K["得到 Process + 主 Thread"]
    K --> L["初始化 ProcManager 和 ThreadManager"]
    L --> M["PROCESSOR.add_proc(pid, process)"]
    M --> N["PROCESSOR.add(tid, thread, pid)"]
    N --> O["进入主调度循环"]
```

ch8 和 ch7 的一个重要区别是：`Process::from_elf` 不再只返回一个进程，而是返回 `(Process, Thread)`。这体现了“资源容器”和“执行单元”的拆分。

## Process / Thread 数据结构链

```mermaid
flowchart TD
    A["process.rs::Process"] --> B["pid"]
    A --> C["address_space"]
    A --> D["fd_table"]
    A --> E["signal"]
    A --> F["semaphore_list"]
    A --> G["mutex_list"]
    A --> H["condvar_list"]

    I["process.rs::Thread"] --> J["tid"]
    I --> K["ForeignContext"]
```

理解口诀：

```text
Process 管共享资源。
Thread 管当前执行。
```

同一进程中的多个线程共享：

- 地址空间。
- 文件描述符表。
- 信号处理状态。
- mutex/semaphore/condvar 列表。

每个线程独立拥有：

- TID。
- 用户栈。
- 寄存器上下文。
- 调度状态。

## Processor 双层管理链

```mermaid
flowchart TD
    A["processor.rs::PROCESSOR"] --> B["ProcessorInner"]
    B --> C["PThreadManager<Process, Thread, ThreadManager, ProcManager>"]
    C --> D["ProcManager"]
    C --> E["ThreadManager"]
    D --> F["BTreeMap<ProcId, Process>"]
    E --> G["BTreeMap<ThreadId, Thread>"]
    E --> H["VecDeque<ThreadId> ready_queue"]
```

`ProcManager` 负责 PID 到进程资源的映射。`ThreadManager` 负责 TID 到线程执行体的映射，并维护就绪队列。`PThreadManager` 则负责把两层对象关联起来，例如当前线程属于哪个进程、某进程有哪些线程、线程退出时是否需要回收进程等。

## 主调度循环链

```mermaid
flowchart TD
    A["main.rs 主循环"] --> B["PROCESSOR.fetch() 取下一个就绪线程"]
    B --> C["ForeignContext::execute(portal)"]
    C --> D["进入用户态线程"]
    D --> E["用户线程执行"]
    E --> F["ecall 或异常返回内核"]
    F --> G["读取 scause"]
    G --> H{"UserEnvCall?"}
    H -- "是" --> I["tg_syscall::handle()"]
    I --> J["根据 syscall id 分发"]
    J --> K{"返回类型"}
    K -- "EXIT" --> L["make_current_exited"]
    K -- "普通 syscall" --> M["写回 a0 并继续"]
    K -- "资源不可用 ret=-1" --> N["make_current_blocked"]
    H -- "异常/页错误" --> O["杀死当前线程/进程"]
```

ch8 的主循环重点是：同步原语可能让线程阻塞，而不是简单返回。对于 `SEMAPHORE_DOWN`、`MUTEX_LOCK`、`CONDVAR_WAIT`，如果返回 `-1`，主循环会调用 `make_current_blocked()`。

## thread_create 调用链

```mermaid
flowchart TD
    A["用户态 thread_create(entry, arg)"] --> B["ecall"]
    B --> C["impl Thread for SyscallContext::thread_create"]
    C --> D["取得当前 Process"]
    D --> E["在地址空间高地址区域搜索空闲用户栈"]
    E --> F["map_extern / map 映射新线程栈"]
    F --> G["LocalContext::user(entry)"]
    G --> H["设置 sp 为新栈顶"]
    H --> I["设置 a0 = arg"]
    I --> J["使用当前进程 satp 创建 Thread"]
    J --> K["PROCESSOR.add(tid, thread, pid)"]
    K --> L["返回 tid 给用户态"]
```

注意：新线程和当前线程共享同一个地址空间，所以它们使用同一个进程的 `satp`。差别在于各自有不同栈和执行上下文。

## waittid 调用链

```mermaid
flowchart TD
    A["用户态 waittid(tid)"] --> B["ecall"]
    B --> C["impl Thread::waittid"]
    C --> D{"等待自己?"}
    D -- "是" --> E["返回 -1"]
    D -- "否" --> F["PROCESSOR.waittid(ThreadId)"]
    F --> G{"目标线程已退出?"}
    G -- "是" --> H["返回 exit_code"]
    G -- "否" --> I["返回 -1 或继续等待策略"]
```

`waittid` 用于回收线程退出状态，类似进程里的 `waitpid`，但粒度变成线程。

## mutex 调用链

```mermaid
flowchart TD
    A["mutex_create(blocking)"] --> B["Process.mutex_list 新增锁"]
    C["mutex_lock(mutex_id)"] --> D["取得当前 tid"]
    D --> E["取 Process.mutex_list[mutex_id]"]
    E --> F["mutex.lock(tid)"]
    F --> G{"成功?"}
    G -- "是" --> H["返回 0"]
    G -- "否" --> I["返回 -1，主循环阻塞当前线程"]
    J["mutex_unlock(mutex_id)"] --> K["mutex.unlock()"]
    K --> L{"有等待线程?"}
    L -- "是" --> M["PROCESSOR.re_enque(tid)"]
    L -- "否" --> N["返回 0"]
```

mutex 属于进程资源，所以所有线程看到的是同一个 `mutex_list`。

## semaphore 调用链

```mermaid
flowchart TD
    A["semaphore_create(count)"] --> B["Process.semaphore_list 新增信号量"]
    C["semaphore_down(sem_id)"] --> D["取当前 tid"]
    D --> E["sem.down(tid)"]
    E --> F{"计数足够?"}
    F -- "是" --> G["计数减一，返回 0"]
    F -- "否" --> H["线程进入等待队列，返回 -1"]
    I["semaphore_up(sem_id)"] --> J["sem.up()"]
    J --> K{"有等待线程?"}
    K -- "是" --> L["唤醒一个 tid"]
    L --> M["PROCESSOR.re_enque(tid)"]
```

semaphore 和 mutex 的共同点是都可能阻塞线程；不同点是 semaphore 管的是计数资源。

## condvar 调用链

```mermaid
flowchart TD
    A["condvar_create()"] --> B["Process.condvar_list 新增条件变量"]
    C["condvar_wait(condvar_id, mutex_id)"] --> D["取当前 tid"]
    D --> E["取 condvar 和 mutex"]
    E --> F["condvar.wait_with_mutex(tid, mutex)"]
    F --> G["释放 mutex 并阻塞当前线程"]
    G --> H{"释放 mutex 是否唤醒其他线程?"}
    H -- "是" --> I["PROCESSOR.re_enque(waking_tid)"]
    J["condvar_signal(condvar_id)"] --> K["condvar.signal()"]
    K --> L{"有等待线程?"}
    L -- "是" --> M["PROCESSOR.re_enque(tid)"]
```

条件变量的关键是：等待时要释放锁，唤醒后还要重新参与同步。

## 死锁检测 exercise 链

```mermaid
flowchart TD
    A["用户态 enable_deadlock_detect(true)"] --> B["syscall 469"]
    B --> C["SyncMutex::enable_deadlock_detect"]
    C --> D["为当前进程开启检测标志"]
    E["mutex_lock / semaphore_down"] --> F{"检测是否开启?"}
    F -- "否" --> G["按普通阻塞逻辑执行"]
    F -- "是" --> H["构造资源分配状态"]
    H --> I["运行安全性/死锁检测"]
    I --> J{"安全?"}
    J -- "是" --> K["允许申请资源"]
    J -- "否" --> L["拒绝申请，返回 -0xDEAD"]
```

exercise.md 给出的算法使用：

```text
Available：每类资源剩余数量。
Allocation：每个线程已持有资源数。
Need：每个线程还需要的资源数。
Work：模拟可用资源。
Finish：模拟线程是否能完成。
```

如果无法找到一个让所有线程完成的安全序列，就认为存在死锁风险。

## ch8_usertest 测试链

```mermaid
flowchart TD
    A["运行 ch8_usertest"] --> B["遍历 TESTS 数组"]
    B --> C["fork 子进程"]
    C --> D["子进程 exec(test)"]
    D --> E["运行 threads / sem / mutex / condvar / deadlock 测试"]
    B --> F["父进程 waitpid"]
    F --> G["收集退出码"]
    G --> H["全部完成后打印 ch8 Usertests passed"]
```

`ch8_usertest.rs` 把线程、同步、管道、死锁检测等测试统一跑一遍，是本章综合回归入口。

## GitHub 交付说明

本文件与同目录其他三份 ch8 文档整理完成后，随提交推送到 GitHub 仓库 `doc/ch8/`。这是对课程要求“Markdown 文档、AI 协作归档、学习进展记录”的补充。
