# rCore ch4 代码链与模块对应底稿

## 目录结构观察

ch4 相比 ch3，最关键的变化是引入地址空间和页表管理。代码上可以分成几条线：

```text
tg-rcore-tutorial-ch4/
├── build.rs
├── src/
│   ├── main.rs
│   ├── process.rs
│   ├── graphics.rs
│   ├── keyboard.rs
│   └── ...
tg-rcore-tutorial-kernel-vm/
├── src/
│   ├── lib.rs
│   └── space/
│       ├── mod.rs
│       ├── mapper.rs
│       └── visitor.rs
tg-rcore-tutorial-user/
└── src/bin/
    └── ch4_tetris.rs
```

其中：

- `main.rs`：内核主流程、地址空间初始化、系统调用实现、调度循环。
- `process.rs`：从 ELF 创建用户进程，建立用户地址空间。
- `kernel-vm`：页表和地址空间抽象。
- `graphics.rs`：ch4 Tetris 扩展中的 VirtIO-GPU 输出。
- `keyboard.rs`：ch4 Tetris 扩展中的 VirtIO-keyboard 输入。
- `ch4_tetris.rs`：用户态俄罗斯方块程序。

## 启动与加载链

```mermaid
flowchart TD
    A["cargo run"] --> B["ch4/build.rs"]
    B --> C["选择 case: 默认 ch4_tetris"]
    C --> D["编译 user/src/bin/ch4_tetris.rs"]
    D --> E["生成 app.asm: incbin 用户 ELF"]
    E --> F["ch4/src/main.rs::rust_main()"]
    F --> G["KernelLayout::locate()"]
    G --> H["初始化 console/log/heap"]
    H --> I["kernel_space() 建立内核地址空间"]
    I --> J["AppMeta::locate().iter()"]
    J --> K["process.rs::Process::new(elf)"]
    K --> L["解析 ELF Program Header"]
    L --> M["AddressSpace::map() 映射 LOAD 段"]
    M --> N["map_extern() 映射用户栈"]
    N --> O["创建 ForeignContext + satp"]
    O --> P["push 到 PROCESSES"]
```

这个链条说明：用户程序不是运行时从磁盘加载的，而是在构建阶段被 `incbin` 打包进内核镜像。内核启动后通过 `AppMeta` 找到这些 ELF 字节，再为每个 ELF 创建独立进程。

## 用户地址空间创建链

```mermaid
flowchart TD
    A["Process::new(elf)"] --> B["读取 ELF64 e_entry"]
    B --> C["读取 Program Header 表"]
    C --> D{"p_type == PT_LOAD?"}
    D -- "否" --> C
    D -- "是" --> E["读取 p_offset/p_vaddr/p_filesz/p_memsz/p_flags"]
    E --> F["计算虚拟页范围"]
    F --> G["根据 R/W/X 构造页表权限"]
    G --> H["AddressSpace::map()"]
    H --> I["分配物理页"]
    I --> J["复制 ELF 段数据"]
    J --> K["建立 VPN -> PPN 映射"]
    K --> L["继续下一个 LOAD 段"]
    L --> M["映射用户栈"]
    M --> N["设置用户 sp"]
    N --> O["构造 satp"]
```

这里的重点不是“复制一个程序”，而是“把 ELF 中各段映射进一个新地址空间”。每个 LOAD 段都有自己的权限，例如代码段可读可执行，数据段可读可写。

## 内核地址空间创建链

```mermaid
flowchart TD
    A["main.rs::kernel_space()"] --> B["遍历 KernelLayout"]
    B --> C["映射 .text/.rodata/.data/.boot"]
    C --> D["映射内核 heap 区间"]
    D --> E["映射 QEMU MMIO 设备地址"]
    E --> F["映射异界传送门 PROTAL_TRANSIT"]
    F --> G["写 satp 开启 Sv39"]
```

本次 ch4 Tetris 的一个关键 bug 就出在这里。开启 Sv39 以后，内核访问设备 MMIO 地址也要经过页表。如果没有把 `0x1000_0000` 附近的 UART、VirtIO-GPU、VirtIO-keyboard 地址映射进去，内核访问 `0x1000_1000` 会触发 `LoadPageFault`。

修复后的设备映射逻辑是：

```text
0x1000_0000 -> UART
0x1000_1000 -> VirtIO-GPU
0x1000_2000 -> VirtIO-keyboard
```

## 系统调用地址翻译链

以 `write(fd, buf, count)` 为例：

```mermaid
flowchart TD
    A["用户态 ch4_tetris 调用 write"] --> B["user/src/syscall.rs::syscall()"]
    B --> C["ecall"]
    C --> D["ForeignContext 返回内核"]
    D --> E["main.rs::schedule()"]
    E --> F["tg_syscall::handle()"]
    F --> G["impl IO for SyscallContext::write()"]
    G --> H["找到当前进程 PROCESSES[caller.entity]"]
    H --> I["process.address_space.translate(buf, READABLE)"]
    I --> J["得到内核可访问指针"]
    J --> K{"fd 是什么?"}
    K -- "STDOUT" --> L["打印字符串"]
    K -- "GRAPHICS_FD=3" --> M["graphics::submit_tetris_frame()"]
    M --> N["解析 TetrisFrame"]
    N --> O["VirtIO-GPU flush"]
```

这条链就是 ch4 的核心：系统调用不再能直接使用用户指针，必须走当前进程的 `AddressSpace::translate()`。

## ch4 Tetris 图形链

```mermaid
flowchart TD
    A["ch4_tetris.rs 游戏循环"] --> B["更新棋盘状态"]
    B --> C["构造 TetrisFrame"]
    C --> D["write(fd=3, frame_bytes)"]
    D --> E["内核 IO::write"]
    E --> F["translate 用户 frame 指针"]
    F --> G["graphics.rs::submit_tetris_frame"]
    G --> H["ensure_gpu 初始化 VirtIO-GPU"]
    H --> I["draw_frame 写 framebuffer"]
    I --> J["gpu.flush()"]
    J --> K["QEMU GTK 窗口显示"]
```

用户态只提交抽象游戏帧，不直接碰硬件。内核态负责把游戏帧翻译成像素并提交给 GPU。

## ch4 Tetris 输入链

```mermaid
flowchart TD
    A["ch4_tetris.rs::try_getchar()"] --> B["read(STDIN, one_byte)"]
    B --> C["ecall 进入内核"]
    C --> D["IO::read()"]
    D --> E["keyboard.rs::take()"]
    E --> F["VirtIOInput::pop_pending_event()"]
    F --> G["keycode_to_ascii()"]
    G --> H["返回 a/d/w/s/space/q"]
    H --> I["用户态更新方块"]
```

键盘输入也体现了用户态/内核态分工：用户程序只读标准输入，具体是 UART 还是 VirtIO-keyboard，由内核实现。

## 测试链

```mermaid
flowchart TD
    A["cargo run"] --> B["默认 ch4_tetris"]
    B --> C["GTK + GPU + keyboard"]
    D["cargo run --features exercise"] --> E["ch4_exercise 测试集"]
    E --> F["headless QEMU runner"]
    G["cargo run --features base"] --> H["ch4 原始基础测试集"]
    H --> I["headless QEMU runner"]
```

为了避免 CI 卡在图形窗口，`test.sh` 会强制使用：

```text
qemu-system-riscv64 -machine virt -nographic -bios none -kernel
```

而本地默认运行使用：

```text
qemu-system-riscv64 -machine virt -display gtk -serial stdio
  -device virtio-gpu-device
  -device virtio-keyboard-device
```

## 本次调试关键点

1. 一开始 `cargo run` 实际打包的不是 `ch4_tetris`，而是原 ch4 测试程序，需要在 `build.rs` 中区分默认 case、base case、exercise case。
2. 用户态程序成功启动后，访问 GPU MMIO 地址触发 `LoadPageFault 0x10001000`。
3. 原因是 ch4 开启页表后，内核地址空间没有映射 VirtIO-GPU 和 VirtIO-keyboard 的 MMIO 页。
4. 修复方式是在 `kernel_space()` 中额外映射 `0x1000_0000..0x1000_3000`。
5. 修复后日志出现 `virtio-gpu ready` 和 `virtio-keyboard ready`，说明设备链路打通。
