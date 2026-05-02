# rCore ch3 代码链与模块对应底稿

本文用“模块在哪里、函数怎么走”的方式整理 ch3。目标不是背代码，而是知道一条执行流经过哪些模块。

## 1. 目录结构重点

```text
tg-rcore-tutorial-ch3/
├── build.rs
├── Cargo.toml
├── test.sh
└── src
    ├── main.rs
    ├── task.rs
    ├── graphics.rs
    └── keyboard.rs

tg-rcore-tutorial-user/
├── cases.toml
└── src
    ├── lib.rs
    └── bin
        ├── ch3_trace.rs
        ├── ch3_snake.rs
        └── ch3_snake_ci.rs
```

模块分工：

```text
build.rs
  选择 cases.toml 里的应用，把用户程序编译并链接进内核。

src/main.rs
  内核入口、主调度循环、系统调用实现、Trap 原因处理、stdin 输入缓存入口。

src/task.rs
  TaskControlBlock、系统调用处理、任务完成状态、系统调用计数。

src/graphics.rs
  ch3-snake 图形输出支持，把用户态 SnakeFrame 画到 VirtIO-GPU framebuffer。

src/keyboard.rs
  ch3-snake 键盘输入支持，从 VirtIO Keyboard 读取 W/A/S/D/Q。

tg-rcore-tutorial-user/src/lib.rs
  用户态最小运行时，提供 println、sleep、try_getchar 等接口。

tg-rcore-tutorial-user/src/bin/ch3_snake.rs
  用户态可交互贪吃蛇。

tg-rcore-tutorial-user/src/bin/ch3_snake_ci.rs
  自动演示贪吃蛇，用于 CI。
```

## 2. 启动到多任务加载

```mermaid
flowchart TD
    A["QEMU 启动内核"] --> B["main.rs::_start"]
    B --> C["main.rs::rust_main"]
    C --> D["zero_bss 清空 BSS"]
    D --> E["tg_console::init_console"]
    E --> F["tg_syscall::init_* 注册 syscall 实现"]
    F --> G["tg_linker::AppMeta::locate() 找到所有 app"]
    G --> H["task.rs::TaskControlBlock::init(entry)"]
    H --> I["主循环 round-robin 调度"]
```

关键点：

```text
build.rs 提前把用户程序作为数据放进内核镜像。
rust_main 运行时通过 AppMeta 找到这些应用。
每个 app 对应一个 TaskControlBlock。
```

## 3. 一个任务被执行的路径

```mermaid
flowchart TD
    A["main.rs 调度循环选中 tcb"] --> B["task.rs::TaskControlBlock::execute"]
    B --> C["tg-kernel-context::LocalContext::execute"]
    C --> D["恢复用户寄存器"]
    D --> E["sret 进入 U-mode"]
    E --> F["用户程序继续运行"]
```

这里的 `execute()` 不是普通函数调用，而是恢复寄存器后通过 RISC-V 特权级返回机制进入用户程序。

## 4. write 系统调用路径：普通字符输出

```mermaid
flowchart TD
    A["用户程序 println!"] --> B["user_lib::Console::put_str"]
    B --> C["tg_syscall::write"]
    C --> D["ecall"]
    D --> E["Trap 回到 main.rs 调度循环"]
    E --> F["task.rs::TaskControlBlock::handle_syscall"]
    F --> G["tg_syscall::handle"]
    G --> H["main.rs::impls::SyscallContext::write"]
    H --> I["tg_console 输出到 SBI console"]
```

重点：

```text
用户态只负责发出 syscall。
真正写终端的是内核里的 SyscallContext::write。
```

## 5. write 系统调用路径：snake 图形输出

```mermaid
flowchart TD
    A["user/bin/ch3_snake.rs::draw"] --> B["打包 SnakeFrame"]
    B --> C["user_lib::write(fd=3, frame_bytes)"]
    C --> D["ecall"]
    D --> E["task.rs::handle_syscall"]
    E --> F["main.rs::SyscallContext::write"]
    F --> G{"fd == GRAPHICS_FD?"}
    G --> H["graphics.rs::submit_snake_frame"]
    H --> I["graphics.rs::ensure_gpu"]
    I --> J["VirtIOGpu::setup_framebuffer"]
    J --> K["graphics.rs::draw_frame"]
    K --> L["VirtIOGpu::flush"]
    L --> M["QEMU GTK 窗口显示"]
```

`fd = 3` 是本实验约定的图形通道。它让用户态仍然走系统调用，而不是直接访问 GPU。

## 6. read 系统调用路径：snake 键盘输入

```mermaid
flowchart TD
    A["用户按 W/A/S/D/Q"] --> B["QEMU virtio-keyboard-device"]
    B --> C["keyboard.rs::refresh"]
    C --> D["VirtIOInput::pop_pending_event"]
    D --> E["evdev keycode 转 ASCII"]
    E --> F["main.rs::input::LAST_KEY"]
    G["user/bin/ch3_snake.rs"] --> H["user_lib::try_getchar"]
    H --> I["user_lib::read(STDIN)"]
    I --> J["ecall"]
    J --> K["main.rs::SyscallContext::read"]
    K --> L["input::take"]
    L --> M["返回 Some(byte) 或 None"]
    M --> N["蛇改变方向或退出"]
```

这个输入实现是轮询式的。它不是完整键盘中断驱动，但已经符合本章扩展目标：用户态通过 `read` 请求内核提供输入。

## 7. yield 调度路径

```mermaid
flowchart TD
    A["用户程序 sched_yield"] --> B["tg_syscall::sched_yield"]
    B --> C["ecall"]
    C --> D["TaskControlBlock::handle_syscall"]
    D --> E["识别 Id::SCHED_YIELD"]
    E --> F["返回 SchedulingEvent::Yield"]
    F --> G["main.rs 主循环 break"]
    G --> H["i = (i + 1) % index_mod"]
    H --> I["切换到下一个未完成任务"]
```

这里的调度逻辑很朴素，是 round-robin 轮转。

## 8. 时钟中断调度路径

```mermaid
flowchart TD
    A["main.rs 设置 tg_sbi::set_timer"] --> B["用户程序运行"]
    B --> C["SupervisorTimer 中断"]
    C --> D["Trap 回到内核"]
    D --> E["main.rs 读取 scause"]
    E --> F["识别 SupervisorTimer"]
    F --> G["tg_sbi::set_timer(u64::MAX) 清掉本轮定时器"]
    G --> H["input::refresh"]
    H --> I["keyboard::refresh 读取 VirtIO Keyboard"]
    I --> J["break 当前任务执行片段"]
    J --> K["调度下一个任务"]
```

本次 ch3-snake 把输入刷新也挂到了时间片边界上。这样即使用户态没有立刻主动读取，内核也会周期性缓存一个按键。

## 9. trace 练习路径

```mermaid
flowchart TD
    A["用户程序 count_syscall / trace_read / trace_write"] --> B["tg_syscall::trace"]
    B --> C["ecall syscall id = 410"]
    C --> D["task.rs::handle_syscall 先统计 syscall_count"]
    D --> E["tg_syscall::handle 分发"]
    E --> F["main.rs::impls::Trace::trace"]
    F --> G{"trace_request"}
    G --> H["0: 读用户地址 1 字节"]
    G --> I["1: 写用户地址 1 字节"]
    G --> J["2: 查询 TCB syscall_count"]
```

关键点：

```text
caller.entity 保存了当前 TaskControlBlock 指针。
trace_request = 2 查询的是当前任务自己的 syscall_count。
```

## 10. snake 和 snake-ci 的构建选择

```mermaid
flowchart TD
    A["cargo run"] --> B["build.rs case_key = ch3"]
    C["cargo run --features exercise"] --> D["case_key = ch3_exercise"]
    E["cargo run --features snake"] --> F["case_key = ch3_snake"]
    G["cargo run --features snake-ci"] --> H["case_key = ch3_snake_ci"]
    B --> I["cases.toml"]
    D --> I
    F --> I
    H --> I
```

这样同一个 ch3 内核可以运行不同的用户程序集合：

```text
base: 原始多任务测试
exercise: trace 作业测试
snake: 人玩的图形贪吃蛇
snake-ci: 自动演示/CI 测试
```

## 11. 常用命令

```powershell
cd C:\Users\FLY\Desktop\OS\Tg-rCore-Tutorial-2026S-git\tg-rcore-tutorial-ch3
$env:Path="C:\Program Files\qemu;$env:Path"

# 基础测试
cargo run

# trace 练习
cargo run --features exercise

# 可交互图形贪吃蛇
cargo run --features snake

# 自动测试贪吃蛇
cargo run --features snake-ci
```

在 GitHub/CNB 的 Linux CI 里，`test.sh` 会自动把 runner 改成 headless 模式：

```bash
./test.sh base
./test.sh exercise
./test.sh snake
```
