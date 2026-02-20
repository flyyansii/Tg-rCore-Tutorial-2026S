# TanGram-rCore-Tutorial

面向操作系统课程教学与自学的组件化 Tangram rCore Tutorial 操作系统内核实验仓库。  
仓库同时包含：

- `tg-rcore-tutorial-ch1~tg-rcore-tutorial-ch8`：tg-rcore-tutorial-ch[1-8] 8 个渐进章节（每章是一个可独立运行的内核 crate + 指导文档）
- `tg-*`：可复用内核组件 crate（内存、虚存、上下文、同步、信号、文件系统等）
- `tg-rcore-tutorial-user`：用户态测试程序集合
- `tg-rcore-tutorial-checker`：测试输出检测工具
- `tg-rcore-tutorial-linker`：为 ch1~ch8的rCore Tutorial教学操作系统内核提供链接脚本生成功能

目标是让你既能按章节学习内核演进，也能按组件视角开发和复用内核模块。

## 1. 先看这段：如何开始

### 1.0 基于Web IDE方式的快速试用
不需要配置开发运行环境，只需有一个能上网的浏览器即可。
 - [教程国内网址](https://cnb.cool/LearningOS/tg-rcore-tutorial/-/tree/test) 
   - 阅读[[豆包提供的基于 cnb 的Web IDE 实践 tg-rcore-tutorial 简易指导书]](https://www.doubao.com/thread/w236fa7686eb1d316)并按其提示操作
 - [教程国外网址](https://github.com/LearningOS/tg-rcore-tutorial/tree/test)
   - 阅读[豆包提供的基于 github 的codespaces Web IDE 实践 tg-rcore-tutorial 简易指导书](https://www.doubao.com/thread/w8fbf39ac661d8907)并按其提示操作

### 1.1 环境要求

- Rust toolchain：本仓库使用 `stable`（见 `rust-toolchain.toml`）
- 目标架构：`riscv64gc-unknown-none-elf`
- 组件：`rust-src`、`llvm-tools-preview`（`rust-toolchain.toml` 已声明）
- QEMU：`qemu-system-riscv64`（建议 >= 7.0）
- 推荐工具：`cargo-binutils`、`cargo-clone`

### 1.2 获取代码

#### 方式 A：直接获取完整实验仓库（推荐）
```bash
git clone https://github.com/rcore-os/tg-rcore-tutorial.git
cd tg-rcore-tutorial
git checkout test #切换到test分支
```

#### 获取某个操作系统内核或内核功能组件（单独 crate）
```bash
cargo clone tg-rcore-tutorial-ch3  #tg-rcore-tutorial-chX 是发布到 crates.io上的组件化内核， X=1..8 代表8个内核 
cd tg-rcore-tutorial-ch3  # 进入 tg-rcore-tutorial-ch3 内核
```

### 1.3 最短上手路径（建议）

```bash
cd tg-rcore-tutorial-ch3
cargo run
```

如果你想直接做基本功能测试（例如 ch3）：
```bash
# 先安装tg-rcore-tutorial-checker`测试输出检测工具
cargo install tg-rcore-tutorial-checker
# 开始测试
cd tg-rcore-tutorial-ch3
cargo build   
./test.sh base
```

如果你想直接做练习章（例如 ch3）：

```bash
cd tg-rcore-tutorial-ch3
cargo build --features exercise
./test.sh exercise
```

## 2. 仓库结构总览

| 路径 | 作用 | 你通常在什么时候用 |
|---|---|---|
| `tg-rcore-tutorial-ch[1-8]` | 8个章节的内核 + 实验指导 | 按课程顺序学习、做章节实验 |
| `tg-rcore-tutorial-console` | 控制台输出与日志 | 需要统一日志/输出接口 |
| `tg-rcore-tutorial-linker` | 链接脚本生成工具 | 构建内核镜像、管理链接符号 |
| `tg-rcore-tutorial-sbi` | SBI 封装（含 `nobios` 支持） | 与固件/定时器/关机交互 |
| `tg-rcore-tutorial-syscall` | syscall 编号与 trait 框架 | 定义/实现系统调用 |
| `tg-rcore-tutorial-kernel-context` | 上下文切换与执行上下文 | Trap、任务/线程切换 |
| `tg-rcore-tutorial-kernel-alloc` | 内核内存分配器 | 需要 `#[global_allocator]` 时 |
| `tg-rcore-tutorial-kernel-vm` | 地址空间与页表管理 | ch4+ 虚存、映射、权限检查 |
| `tg-rcore-tutorial-task-manage` | 任务/进程/线程管理抽象 | 调度器与任务关系管理 |
| `tg-rcore-tutorial-easy-fs` | 教学文件系统实现 | ch6+ 文件、目录、pipe |
| `tg-rcore-tutorial-sync` | 同步原语（mutex/semaphore/condvar） | ch8 并发同步 |
| `tg-rcore-tutorial-signal-defs` | 信号号与结构定义 | ch7+ 信号语义定义 |
| `tg-rcore-tutorial-signal` | 信号处理 trait 抽象 | 信号框架扩展点 |
| `tg-rcore-tutorial-signal-impl` | 信号处理具体实现 | 直接复用信号实现 |
| `tg-rcore-tutorial-user` | 用户程序与测试用例 | 内核构建期打包/拉取用户态测例 |
| `tg-rcore-tutorial-checker` | 输出匹配检测工具 | 自动判定章节测试是否通过 |

## 3. 章节与练习地图

| 章节 | 主题 | 默认运行 | 练习模式 |
|---|---|---|---|
| `tg-rcore-tutorial-ch1` | 裸机与最小执行环境 | `cargo run` | 无独立 exercise |
| `tg-rcore-tutorial-ch2` | Batch OS、Trap、基本 syscall | `cargo run` | 无独立 exercise |
| `tg-rcore-tutorial-ch3` | 多道程序与分时 | `cargo run` | `cargo run --features exercise` |
| `tg-rcore-tutorial-ch4` | 地址空间与页表 | `cargo run` | `cargo run --features exercise` |
| `tg-rcore-tutorial-ch5` | 进程与调度 | `cargo run` | `cargo run --features exercise` |
| `tg-rcore-tutorial-ch6` | 文件系统 | `cargo run` | `cargo run --features exercise` |
| `tg-rcore-tutorial-ch7` | IPC（pipe/signal） | `cargo run` | 基础测试为主 |
| `tg-rcore-tutorial-ch8` | 线程与并发同步 | `cargo run` | `cargo run --features exercise` |

5 个常见练习章：`tg-rcore-tutorial-ch[34568]`。

<a id="chapters-source-nav-map"></a>

### 3.1 tg-rcore-tutorial-ch[1-8] 源码导航总表（配套注释版）

下面这张表用于跨章节快速定位源码阅读入口；每章 README 里还有更细的“源码阅读导航索引”。

| 章节（点击直达导航） | 建议先读的源码文件（顺序） | 关注主线 |
|---|---|---|
| [`ch1`](tg-rcore-tutorial-ch1/README.md#source-nav) | `src/main.rs` | 裸机最小启动：`_start -> rust_main -> panic` |
| [`ch2`](tg-rcore-tutorial-ch2/README.md#source-nav) | `src/main.rs` | 批处理 + Trap + syscall 分发 |
| [`ch3`](tg-rcore-tutorial-ch3/README.md#source-nav) | `src/task.rs` -> `src/main.rs` | 任务模型 + 抢占/协作调度 |
| [`ch4`](tg-rcore-tutorial-ch4/README.md#source-nav) | `src/main.rs` -> `src/process.rs` | 页表与地址空间 + `translate` |
| [`ch5`](tg-rcore-tutorial-ch5/README.md#source-nav) | `src/process.rs` -> `src/processor.rs` -> `src/main.rs` | `fork/exec/wait` 与进程关系管理 |
| [`ch6`](tg-rcore-tutorial-ch6/README.md#source-nav) | `src/virtio_block.rs` -> `src/fs.rs` -> `src/main.rs` | 块设备到文件系统，再到 fd 系统调用 |
| [`ch7`](tg-rcore-tutorial-ch7/README.md#source-nav) | `src/fs.rs` -> `src/process.rs` -> `src/main.rs` | 管道统一 fd 抽象 + 信号处理 |
| [`ch8`](tg-rcore-tutorial-ch8/README.md#source-nav) | `src/process.rs` -> `src/processor.rs` -> `src/main.rs` | 线程化调度 + 同步原语阻塞/唤醒 |

## 4. 常用开发与测试流程

### 4.1 进入某章开发

```bash
cd tg-rcore-tutorial-ch<N>
cargo build
cargo run
```

### 4.2 运行章节测试脚本

```bash
./test.sh          # 默认（通常等价于 all 或 base）
./test.sh base     # 基础测试
./test.sh exercise # 练习测试（若该章支持）
./test.sh all      # 全量测试（若该章支持）
```

### 4.3 使用 `tg-rcore-tutorial-checker` 做输出检测

先安装（本地路径）：

```bash
cargo install --path tg-rcore-tutorial-checker
```

基础测试示例（以 tg-rcore-tutorial-ch2 为例）：

```bash
cargo run 2>&1 | tg-rcore-tutorial-checker --ch 2
```

练习测试示例（以 ch3 为例）：

```bash
cargo run --features exercise 2>&1 | tg-rcore-tutorial-checker --ch 3 --exercise
```

## 5. `tg-*` 内核功能组件开发工作流（开发者重点）

### 5.1 本地修改组件后如何验证

建议流程：

1. 在内核功能组件的目录先验证组件本身：
   ```bash
   cd tg-rcore-tutorial-sbi
   cargo check 
   ```
2. 测试依赖该组件的OS，做集成验证：
   ```bash
   cd tg-rcore-tutorial-sbi
   ./systest.sh -l  #测试依赖本地修改后的组件的各个内核是否能通过它们自身测试
   # 如果测试在crates.io上的组件是否能通过测试，则执行  ./systest.sh
   # 可通过修改 systest.txt 和sysdeps.txt 来灵活调整测试对象和测试本地修改的组件
   ```

## 6. 推荐学习/开发顺序

1. `tg-rcore-tutorial-ch1 -> tg-rcore-tutorial-ch2`：跑通启动、Trap、基础 syscall
2. `tg-rcore-tutorial-ch3 -> tg-rcore-tutorial-ch4`：完成任务调度到地址空间
3. `tg-rcore-tutorial-ch5 -> tg-rcore-tutorial-ch6`：完成进程与文件系统
4. `tg-rcore-tutorial-ch7 -> tg-rcore-tutorial-ch8`：完成 IPC、线程与并发同步
5. 回到 `tg-rcore-tutorial-*`：按组件抽象复盘与重构

## 7. 常见问题（FAQ）

### Q1：为什么我在根目录 `cargo run` 不会直接跑某个章节的内核？

因为章节 crate 不在根 workspace 默认成员里。请进入 `tg-rcore-tutorial-ch<N>` 目录运行。

### Q2：为什么 exercise 测试和 base 测试结果不同？

`exercise` 会启用章节额外需求（例如新增 syscall 或扩展行为），与基础模式测例不同是正常的。

### Q3：如何快速定位“实现错了还是输出格式错了”？

先使用章节 `./test.sh`，再用 `tg-rcore-tutorial-checker` 管线检测输出，可快速区分行为错误与输出不匹配。

## 8. 相关文档入口

- 章节文档：`tg-rcore-tutorial-ch[1-8]/README.md`
- 练习说明：`tg-rcore-tutorial-ch[34568]/exercise.md`

## 9. 高频错误速查表（学生版）

> 使用方法：先按“现象”定位，再执行“快速定位命令”，最后按“优先修复动作”处理。

| 现象 | 常见原因 | 快速定位命令 | 优先修复动作 |
|---|---|---|---|
| `can't find crate for core` 或目标不支持 | 未安装 RISC-V 目标 | `rustup target list --installed | rg riscv64gc-unknown-none-elf` | `rustup target add riscv64gc-unknown-none-elf` |
| `qemu-system-riscv64: command not found` | QEMU 未安装或不在 PATH | `qemu-system-riscv64 --version` | 安装 `qemu-system-misc`（Linux）或 `qemu`（macOS） |
| 在仓库根目录 `cargo run` 失败 | 章节 crate 不在根 workspace 默认成员 | `pwd`（确认当前目录） | 进入具体章节目录后再 `cargo run`，如 `cd ch4` |
| `cargo clone: command not found` | 缺少构建依赖工具 | `cargo clone --version` | `cargo install cargo-clone` |
| 构建阶段找不到 `rust-objcopy` | 缺少 `cargo-binutils/llvm-tools` | `rust-objcopy --version` | `cargo install cargo-binutils && rustup component add llvm-tools` |
| ch6+ 运行时报块设备/镜像相关错误 | `fs.img` 未生成或路径不匹配 | `test -f target/riscv64gc-unknown-none-elf/debug/fs.img && echo ok || echo missing` | 在对应章节先 `cargo build`，再 `cargo run` |
| 日志出现 `unsupported syscall` | syscall 未注册或用户/内核接口不一致 | `LOG=trace cargo run` | 检查 `tg_syscall::init_*` 与对应 `impls` 是否已实现并初始化 |
| 运行中出现 `page fault` / `stval` 异常 | 用户指针未翻译、权限标志不匹配、映射缺失 | `LOG=trace cargo run` 并关注 trap 日志 | 优先检查 `translate()` 调用、`VmFlags` 权限、`map/unmap` 范围 |
| `base` 能过但 `exercise` 失败 | 练习功能未实现或 feature 开关不一致 | `./test.sh base && ./test.sh exercise` | 对照 `exercise.md` 完成功能后使用 `--features exercise` 回归 |
| 测试输出看似正确但仍判失败 | 输出格式与 checker 期望不一致 | `cargo run 2>&1 | tg-rcore-tutorial-checker --ch <N> [--exercise]` | 先修行为再修日志格式，避免额外杂项输出污染 |

---

如果你是课程开发者，建议先读完本 README，把`tg-rcore-tutorial-ch[1-8]`的README.md看看，并运行一下，再从 `tg-rcore-tutorial-ch3` 或 `tg-rcore-tutorial-ch4` 开始做一个完整练习闭环（实现 -> 测试 -> 回归 -> 文档化），可以最快理解本仓库的“章节驱动 + 组件复用”开发模式。

## License: GNU GENERAL PUBLIC LICENSE v3