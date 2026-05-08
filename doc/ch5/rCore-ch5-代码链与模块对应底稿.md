# rCore ch5 代码链与模块对应底稿

## 目录结构观察

本章的组件化仓库结构如下：

```text
tg-rcore-tutorial-ch5/
├── build.rs
├── Cargo.toml
├── test.sh
├── .cargo/
│   └── config.toml
└── src/
    ├── main.rs
    ├── process.rs
    ├── processor.rs
    ├── graphics.rs
    └── keyboard.rs

tg-rcore-tutorial-user/
├── cases.toml
└── src/bin/
    ├── ch5_usertest.rs
    ├── ch5_spawn0.rs
    ├── ch5_spawn1.rs
    ├── ch5_setprio.rs
    ├── ch5_stride*.rs
    └── ch5_pingpong.rs
```

相比 Guide 的传统结构，这个组件化版本没有把 `syscall/process.rs`、`syscall/mm.rs`、`task/mod.rs`、`mm/memory_set.rs` 都放在本章目录里，而是做了拆分：

```text
Guide 中的 syscall/process.rs
-> ch5/src/main.rs::impls::Process

Guide 中的 syscall/mm.rs
-> ch5/src/main.rs::impls::Memory

Guide 中的 syscall/fs.rs 或 console I/O
-> ch5/src/main.rs::impls::IO

Guide 中的 TaskManager / Processor
-> ch5/src/processor.rs::ProcManager + PROCESSOR

Guide 中的 ProcessControlBlock
-> ch5/src/process.rs::Process

Guide 中的 MemorySet
-> tg-rcore-tutorial-kernel-vm::AddressSpace

Guide 中的 frame_allocator
-> tg-rcore-tutorial-kernel-alloc

Guide 中的 loader
-> build.rs + APPS Lazy map
```

所以读组件化代码时，不能机械按照 Guide 的文件名找，而要按照功能找。

## ch5 的核心文件职责

### build.rs

`build.rs` 是构建期脚本，负责把用户程序编译并嵌入内核。

它做几件事：

1. 找到 `tg-rcore-tutorial-user`。
2. 读取 `cases.toml`。
3. 根据 feature 和环境变量选择 case。
4. 编译对应用户态程序。
5. 生成 `app.asm`。
6. 让内核编译时通过 `APP_ASM` 环境变量包含这些用户程序。

当前选择逻辑：

```text
--features exercise -> ch5_exercise
CHAPTER=-5          -> ch5
默认 cargo run      -> ch5_pingpong
```

### main.rs

`main.rs` 是内核主控文件，包含：

- `rust_main` 启动流程。
- 内核地址空间建立。
- APPS 表。
- portal 映射。
- 系统调用 trait 实现。
- 初始进程选择。

这里的 `impls` 模块尤其重要，它把 `tg_syscall` 的 trait 接到具体实现上。

### process.rs

`process.rs` 定义进程对象 `Process`。

它包含：

- 从 ELF 创建进程。
- fork 复制进程。
- exec 替换当前进程内容。
- sbrk 调整堆。
- 进程字段如 pid、address_space、context、priority、stride。

### processor.rs

`processor.rs` 定义进程管理器。

它负责：

- 保存所有进程。
- 保存 ready queue。
- 记录当前进程。
- 添加进程。
- 让当前进程挂起。
- 让当前进程退出。
- 使用 stride 算法选择下一个进程。

### graphics.rs

`graphics.rs` 是 pingpong 扩展加入的 VirtIO-GPU 渲染模块。

它负责：

- 初始化 VirtIO-GPU。
- 建立 framebuffer。
- 接收用户态 `PingpongFrame`。
- 绘制球场、挡板、球和比分。
- flush framebuffer。

### keyboard.rs

`keyboard.rs` 是 pingpong 扩展加入的 VirtIO-keyboard 输入模块。

它负责：

- 初始化 VirtIO-keyboard。
- 轮询按键事件。
- 把 QEMU 键码转换为 `w/s/i/k/q`。
- 提供给 `main.rs::input` 统一读取。

## 构建期流程：用户程序如何进入内核

```mermaid
flowchart TD
    A["cargo run / cargo run --features exercise"] --> B["ch5/build.rs"]
    B --> C{"选择 case"}
    C --> D["默认: ch5_pingpong"]
    C --> E["exercise: ch5_exercise"]
    C --> F["CHAPTER=-5: ch5"]
    D --> G["编译 user/src/bin/ch5_pingpong.rs"]
    E --> H["编译 ch5_usertest 和所有练习测例"]
    F --> I["编译 ch5b_usertest 和基础测例"]
    G --> J["生成 app.asm"]
    H --> J
    I --> J
    J --> K["global_asm include APP_ASM"]
    K --> L["用户 ELF 字节被嵌入内核镜像"]
```

这里要注意：用户程序不是运行时从磁盘加载的，而是构建时打包进内核的。

## 启动期 30 步流程

下面是 ch5 从 QEMU 启动到运行第一个用户进程的完整流程。

```text
01. cargo run 调用 Rust 编译。
02. build.rs 先运行。
03. build.rs 读取 TG_USER_DIR，找到用户态 crate。
04. build.rs 读取 cases.toml。
05. build.rs 根据 feature/CHAPTER 选择 case。
06. build.rs 编译对应用户态 ELF。
07. build.rs 生成 app.asm。
08. 内核编译时 include app.asm。
09. QEMU 加载内核 ELF。
10. CPU 进入内核入口。
11. rust_main 开始执行。
12. 清空 .bss。
13. 初始化 console。
14. 设置 log level。
15. 初始化内核堆。
16. 分配 portal 页面。
17. kernel_space 建立内核地址空间。
18. 映射内核 text/rodata/data/boot。
19. 映射内核 heap。
20. 映射 UART / VirtIO-GPU / VirtIO-keyboard MMIO。
21. 映射 portal 页面。
22. 写 satp，开启 Sv39。
23. 初始化 MultislotPortal。
24. 注册 IO/Process/Scheduling/Clock/Memory syscall 实现。
25. 根据 CHAPTER 选择初始进程名。
26. APPS.get 找到初始 ELF。
27. Process::from_elf 解析 ELF 并创建地址空间。
28. map_portal 将 portal 映射复制进用户地址空间。
29. ProcManager::add 把初始进程加入管理器。
30. schedule 循环开始调度用户进程。
```

## 初始进程选择链

我们对 ch5 做了入口区分，避免默认游戏和测试互相干扰。

```mermaid
flowchart TD
    A["main.rs::rust_main"] --> B{"option_env!(CHAPTER)"}
    B -->|Some(\"5\")| C["initproc_name = ch5_usertest"]
    B -->|Some(\"-5\")| D["initproc_name = ch5b_usertest"]
    B -->|None/Other| E["initproc_name = ch5_pingpong"]
    C --> F["APPS.get(initproc_name)"]
    D --> F
    E --> F
    F --> G["ElfFile::new"]
    G --> H["Process::from_elf"]
    H --> I["ProcManager::add"]
```

这样：

- 测试入口固定，不会卡在 shell。
- 默认入口直接进入 pingpong。
- base 和 exercise 分离。

## Process::from_elf 流程

`Process::from_elf` 是从 ELF 创建进程的关键。

```text
01. 输入 ElfFile。
02. 读取 ELF entry。
03. 创建新的 AddressSpace。
04. 遍历 Program Header。
05. 找到 PT_LOAD 段。
06. 读取 p_vaddr/p_offset/p_filesz/p_memsz。
07. 根据 ELF flags 计算 R/W/X 权限。
08. 计算虚拟页范围。
09. AddressSpace 分配物理页。
10. 建立 VPN -> PPN 映射。
11. 复制 ELF 文件内容到物理页。
12. 对 bss 区域清零。
13. 分配用户栈。
14. 设置用户 sp。
15. 设置 heap_bottom 和 program_brk。
16. 创建 ForeignContext。
17. 写入入口地址 entry。
18. 写入用户栈指针。
19. 设置 satp 为该进程地址空间根页表。
20. 初始化 pid/parent/children/priority/stride。
```

## fork 调用链

```mermaid
flowchart TD
    A["user program: fork()"] --> B["tg_syscall::fork"]
    B --> C["ecall"]
    C --> D["portal/trap 回到内核"]
    D --> E["tg_syscall::handle"]
    E --> F["main.rs::impls::Process::fork"]
    F --> G["PROCESSOR.current()"]
    G --> H["process.rs::Process::fork"]
    H --> I["复制父进程地址空间"]
    I --> J["复制上下文"]
    J --> K["设置子进程 fork 返回值为 0"]
    K --> L["ProcManager::add(parent_pid, child)"]
    L --> M["父进程返回 child_pid"]
```

fork 后父子进程从同一位置继续执行，是因为子进程复制了父进程当前上下文。差别靠返回值区分。

## exec 调用链

```mermaid
flowchart TD
    A["user program: exec(path)"] --> B["tg_syscall::exec"]
    B --> C["ecall"]
    C --> D["main.rs::impls::Process::exec"]
    D --> E["translate 用户 path 指针"]
    E --> F["APPS.get(path)"]
    F --> G["ElfFile::new"]
    G --> H["process.rs::Process::exec"]
    H --> I["创建新的 AddressSpace"]
    I --> J["加载新 ELF"]
    J --> K["替换当前 context/address_space"]
    K --> L["保留 PID 和父子关系"]
    L --> M["返回后进入新程序入口"]
```

exec 的关键：换程序，不换 PID。

## wait 调用链

```mermaid
flowchart TD
    A["parent: waitpid(pid, code_ptr)"] --> B["ecall"]
    B --> C["main.rs::impls::Process::wait"]
    C --> D["检查当前进程 children"]
    D --> E{"目标子进程存在?"}
    E -->|否| F["返回 -1"]
    E -->|是| G{"子进程 Zombie?"}
    G -->|否| H["返回 -2 / 让出 CPU 后重试"]
    G -->|是| I["读取 exit_code"]
    I --> J["translate code_ptr"]
    J --> K["写回退出码"]
    K --> L["ProcManager 移除子进程"]
    L --> M["从 children 删除 PID"]
    M --> N["返回子 PID"]
```

wait 的重点是资源回收。

## exit 调用链

```mermaid
flowchart TD
    A["user: exit(code)"] --> B["ecall"]
    B --> C["main.rs::impls::Process::exit"]
    C --> D["返回 exit_code"]
    D --> E["调度循环识别当前进程退出"]
    E --> F["ProcManager::make_current_exited"]
    F --> G["标记 Zombie"]
    G --> H["保存 exit_code"]
    H --> I["不再放回 ready_queue"]
    I --> J["等待父进程 wait 回收"]
```

exit 后进程不是立刻消失，而是等待父进程回收。

## spawn 练习实现链

```mermaid
flowchart TD
    A["user: spawn(path, count)"] --> B["ecall: syscall id 400"]
    B --> C["main.rs::impls::Process::spawn"]
    C --> D["PROCESSOR.get_mut()"]
    D --> E["current() 获取当前进程"]
    E --> F["translate path 用户虚拟地址"]
    F --> G["from_raw_parts(ptr, count)"]
    G --> H["from_utf8_unchecked 得到程序名"]
    H --> I["APPS.get(name)"]
    I --> J["ElfFile::new"]
    J --> K["ProcStruct::from_elf"]
    K --> L["manager.add(parent_pid, child_proc)"]
    L --> M["返回 child_pid"]
```

spawn 和 fork 的区别：

```text
fork: 复制当前进程
spawn: 直接从目标 ELF 创建新进程
```

## mmap 调用链

```mermaid
flowchart TD
    A["user: mmap(start, len, prot)"] --> B["ecall"]
    B --> C["main.rs::impls::Memory::mmap"]
    C --> D["检查 start 页对齐"]
    D --> E["检查 len 和 prot"]
    E --> F["prot_to_flags"]
    F --> G["计算 VPN 范围"]
    G --> H["PROCESSOR.current().address_space"]
    H --> I["逐页检查是否已映射"]
    I --> J{"有冲突?"}
    J -->|是| K["返回 -1"]
    J -->|否| L["address_space.map"]
    L --> M["返回 0"]
```

## munmap 调用链

```mermaid
flowchart TD
    A["user: munmap(start, len)"] --> B["ecall"]
    B --> C["main.rs::impls::Memory::munmap"]
    C --> D["检查 start 页对齐"]
    D --> E["计算 VPN 范围"]
    E --> F["PROCESSOR.current().address_space"]
    F --> G["逐页检查是否已映射"]
    G --> H{"存在未映射页?"}
    H -->|是| I["返回 -1"]
    H -->|否| J["address_space.unmap"]
    J --> K["返回 0"]
```

## set_priority 调用链

```mermaid
flowchart TD
    A["user: set_priority(prio)"] --> B["ecall: syscall id 140"]
    B --> C["main.rs::impls::Scheduling::set_priority"]
    C --> D{"prio >= 2?"}
    D -->|否| E["返回 -1"]
    D -->|是| F["PROCESSOR.current().priority = prio"]
    F --> G["返回 prio"]
```

## stride 调度 28 步细化流程

```text
01. 一个进程进入 ready_queue。
02. 每个进程保存 priority。
03. 每个进程保存 stride。
04. 初始 priority = 16。
05. 初始 stride = 0。
06. 调度器需要选择下一个进程。
07. ProcManager::fetch 被调用。
08. fetch 遍历 ready_queue。
09. 对每个 PID 找到对应 Process。
10. 读取 Process.stride。
11. 记录当前最小 stride。
12. 如果 stride 更小，更新候选进程。
13. 如果 stride 相同，用 PID 做稳定比较。
14. 遍历结束后得到 selected_pid。
15. 从 ready_queue 删除 selected_pid。
16. 找到 selected_pid 对应 Process。
17. 计算 pass = BIG_STRIDE / priority。
18. 如果 pass 为 0，则修正为 1。
19. Process.stride += pass。
20. selected_pid 成为当前运行进程。
21. 用户进程运行一个时间片或直到 syscall。
22. 如果进程 yield，保存状态。
23. yield 后进程重新进入 ready_queue。
24. 如果进程 exit，不再进入 ready_queue。
25. 下一次调度再次调用 fetch。
26. stride 小的进程优先被选。
27. priority 高的进程 pass 小，stride 增长慢。
28. 因此 priority 高的进程获得更多 CPU 机会。
```

## ProcManager 状态迁移链

```mermaid
flowchart TD
    A["Ready"] --> B["fetch 选中"]
    B --> C["Running"]
    C --> D{"syscall / trap 结果"}
    D -->|yield / sleep / wait 未完成| E["Suspend"]
    E --> F["重新放回 ready_queue"]
    F --> A
    D -->|exit / fault| G["Zombie 或删除"]
    D -->|fork/spawn| H["创建子进程 Ready"]
    H --> A
    D -->|exec| I["替换当前地址空间"]
    I --> C
```

## 用户态 pingpong 到内核图形链

```mermaid
flowchart TD
    A["user/bin/ch5_pingpong.rs"] --> B["更新球/挡板/比分"]
    B --> C["构造 PingpongFrame"]
    C --> D["write(fd=3, frame_bytes)"]
    D --> E["ecall"]
    E --> F["main.rs::impls::IO::write"]
    F --> G{"fd == GRAPHICS_FD?"}
    G -->|是| H["translate 用户 frame 指针"]
    H --> I["graphics::submit_pingpong_frame"]
    I --> J["检查 magic"]
    J --> K["ensure_gpu 初始化 VirtIO-GPU"]
    K --> L["draw_frame 写 framebuffer"]
    L --> M["gpu.flush"]
    M --> N["QEMU GTK 窗口显示画面"]
```

## 用户态 pingpong 键盘输入链

```mermaid
flowchart TD
    A["user: try_getchar()"] --> B["read(stdin, one_byte)"]
    B --> C["ecall"]
    C --> D["main.rs::impls::IO::read"]
    D --> E["translate 用户 buffer"]
    E --> F["input::take"]
    F --> G["keyboard::take"]
    G --> H["keyboard::refresh"]
    H --> I["VirtIOInput::pop_pending_event"]
    I --> J["keycode_to_ascii"]
    J --> K["返回 w/s/i/k/q"]
    K --> L["写回用户 buffer"]
    L --> M["用户程序更新挡板"]
```

## ch5_pingpong 用户程序内部 30 步流程

```text
01. 用户程序 ch5_pingpong 作为初始进程启动。
02. 初始化左右挡板位置。
03. 初始化球位置。
04. 初始化速度向量 vx/vy。
05. 初始化比分。
06. 打印控制说明。
07. 构造第一帧 PingpongFrame。
08. write(fd=3) 提交第一帧。
09. 进入游戏循环。
10. try_getchar 轮询按键。
11. 如果按 w，左挡板上移。
12. 如果按 s，左挡板下移。
13. 如果按 i，右挡板上移。
14. 如果按 k，右挡板下移。
15. 如果按 q，提交 game_over 帧并退出。
16. 读取当前时间 get_time。
17. 判断是否到达下一帧时间。
18. 更新 ball_x。
19. 更新 ball_y。
20. 检查上墙/下墙碰撞。
21. 检查左挡板碰撞。
22. 检查右挡板碰撞。
23. 碰撞后反向 vx。
24. 根据碰撞提高 speed。
25. 检查球是否越过左边界。
26. 如果越界，右方得分并重置球。
27. 检查球是否越过右边界。
28. 如果越界，左方得分并重置球。
29. 构造新帧并 write(fd=3)。
30. sleep + sched_yield 让出 CPU。
```

## 图形 DMA 调试链

一开始出现：

```text
[ch5-pingpong] failed to initialize virtio-gpu
```

定位过程：

```text
01. 日志显示 Device features 能读到。
02. 说明 MMIO 地址映射基本正确。
03. 日志显示 Config 能读到。
04. 说明 VirtIO-GPU 设备存在。
05. 失败发生在 setup_framebuffer。
06. framebuffer 大小 = width * height * 4。
07. 800 * 480 * 4 ≈ 1.46MB。
08. 128 页 DMA = 128 * 4096 = 512KB。
09. DMA 不够，setup_framebuffer 失败。
10. 不能简单无限加大 DMA。
11. 因为 ch5 exercise spawn 会创建很多进程。
12. 过大的静态 DMA 会压缩内核可用堆。
13. 解决方案：降低分辨率。
14. 改为 640 * 360 * 4 ≈ 900KB。
15. GPU DMA 调为 256 页 ≈ 1MB。
16. keyboard DMA 保持 16 页。
17. 重新运行后出现 virtio-gpu ready。
```

## 测试链

### exercise 测试

```text
CHAPTER=5 cargo run --features exercise
```

链路：

```text
build.rs 选择 ch5_exercise
-> 初始进程选择 ch5_usertest
-> ch5_usertest 依次 spawn 各测例
-> 测试 mmap/munmap/spawn/set_priority/stride
-> 输出 ch5 Usertests passed!
```

### base 测试

```text
CHAPTER=-5 cargo run
```

链路：

```text
build.rs 选择 ch5
-> 初始进程选择 ch5b_usertest
-> 运行基础 fork/exec/wait/sbrk 等测例
-> 输出 Basic usertests passed!
```

### 默认游戏

```text
cargo run
```

链路：

```text
build.rs 选择 ch5_pingpong
-> 初始进程选择 ch5_pingpong
-> 用户态游戏循环
-> fd=3 图形输出
-> stdin 键盘输入
```

## 本章模块关系总图

```mermaid
flowchart TD
    A["build.rs"] --> B["cases.toml"]
    B --> C["user ELF"]
    C --> D["APP_ASM"]
    D --> E["main.rs::APPS"]
    E --> F["main.rs::rust_main"]
    F --> G["process.rs::Process::from_elf"]
    G --> H["kernel-vm::AddressSpace"]
    G --> I["process.rs::Process"]
    I --> J["processor.rs::ProcManager"]
    J --> K["schedule loop"]
    K --> L["main.rs::impls::Process"]
    K --> M["main.rs::impls::Memory"]
    K --> N["main.rs::impls::Scheduling"]
    K --> O["main.rs::impls::IO"]
    O --> P["graphics.rs"]
    O --> Q["keyboard.rs"]
```

## 读代码时的建议

如果只看 `main.rs` 会觉得它很大，但可以拆成几块：

```text
启动部分:
    rust_main / kernel_space / map_portal

数据入口:
    APPS / app_names

系统调用:
    impls::IO
    impls::Process
    impls::Scheduling
    impls::Clock
    impls::Memory

进程结构:
    process.rs

调度结构:
    processor.rs

游戏扩展:
    graphics.rs
    keyboard.rs
```

真正理解 ch5 的关键是把 `Process` 和 `ProcManager` 分开：

- `Process` 是单个进程档案。
- `ProcManager` 是管理所有进程的调度器和进程表。

## 本章最值得回看的一条链

```text
用户程序 spawn("ch5_getpid")
-> syscall id 400
-> 内核翻译用户 path 地址
-> APPS 找到 ch5_getpid ELF
-> Process::from_elf 创建新地址空间和上下文
-> ProcManager::add 建立父子关系
-> ready_queue 中出现新 PID
-> stride fetch 选中该子进程
-> 子进程运行并 exit
-> 父进程 wait 回收
```

这条链把 ch4 地址空间、ch5 进程管理、系统调用、调度、父子关系全部串起来了。
