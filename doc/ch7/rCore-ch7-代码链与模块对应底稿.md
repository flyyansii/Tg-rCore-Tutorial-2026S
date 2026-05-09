# rCore ch7 代码链与模块对应底稿

## 1. 代码树

```text
tg-rcore-tutorial-ch7/
├── build.rs
├── test.sh
├── .cargo/config.toml
└── src/
    ├── main.rs
    ├── fs.rs
    ├── process.rs
    ├── processor.rs
    ├── virtio_block.rs
    ├── graphics.rs
    └── keyboard.rs

tg-rcore-tutorial-user/
└── src/bin/
    ├── pipetest.rs
    ├── pipe_large_test.rs
    ├── ch7b_usertest.rs
    ├── user_shell.rs
    ├── initproc.rs
    └── ch7_pacman.rs
```

## 2. Guide 和组件化仓库对应

```text
Guide: fs/pipe.rs
-> tg-easy-fs 中 PipeReader/PipeWriter/UserBuffer
-> ch7/src/fs.rs 中 Fd::PipeRead/Fd::PipeWrite

Guide: syscall/fs.rs
-> ch7/src/main.rs::impls::IO

Guide: task/process.rs
-> ch7/src/process.rs::Process

Guide: shell/redirection
-> user_shell + fd_table 继承/替换

Pacman 扩展
-> ch7/src/graphics.rs
-> ch7/src/keyboard.rs
-> user/src/bin/ch7_pacman.rs
```

## 3. fd_table 类型变化

ch6：

```text
fd_table: Vec<Option<Mutex<FileHandle>>>
```

ch7：

```text
fd_table: Vec<Option<Mutex<Fd>>>
```

原因是 fd 不再只可能是普通文件。

## 4. Fd 枚举

```text
Fd::File(FileHandle)
Fd::PipeRead(PipeReader)
Fd::PipeWrite(Arc<PipeWriter>)
Fd::Empty { read, write }
```

`Fd::read` 和 `Fd::write` 负责根据类型调用不同实现。

## 5. pipe 调用链

```mermaid
flowchart TD
    A["user pipe(pipe_fd)"] --> B["ecall"]
    B --> C["main.rs::IO::pipe"]
    C --> D["make_pipe()"]
    D --> E["PipeReader"]
    D --> F["PipeWriter"]
    E --> G["fd_table push Fd::PipeRead"]
    F --> H["fd_table push Fd::PipeWrite"]
    G --> I["write read_fd to user pipe[0]"]
    H --> J["write write_fd to user pipe[1]"]
```

## 6. pipe write 调用链

```mermaid
flowchart TD
    A["user write(write_fd, buf)"] --> B["IO::write"]
    B --> C["translate user buf"]
    C --> D["fd_table[write_fd]"]
    D --> E["Fd::PipeWrite"]
    E --> F["PipeWriter::write"]
    F --> G["ring buffer tail 写入"]
    G --> H["满则返回 -2 或等待重试"]
```

## 7. pipe read 调用链

```mermaid
flowchart TD
    A["user read(read_fd, buf)"] --> B["IO::read"]
    B --> C["translate user buf"]
    C --> D["fd_table[read_fd]"]
    D --> E["Fd::PipeRead"]
    E --> F["PipeReader::read"]
    F --> G["ring buffer head 读出"]
    G --> H["空则返回 -2 或 EOF"]
```

## 8. fork 继承 fd_table

```mermaid
flowchart TD
    A["parent fd_table"] --> B["fork"]
    B --> C["child fd_table clone"]
    C --> D["File / Pipe Arc 引用共享"]
    D --> E["父子可通过同一 pipe 通信"]
```

## 9. 重定向链

```mermaid
flowchart TD
    A["shell 解析 > out"] --> B["open out"]
    B --> C["关闭 stdout fd=1"]
    C --> D["dup/open 到 fd=1"]
    D --> E["exec target"]
    E --> F["target write(1, data)"]
    F --> G["写入 out 文件"]
```

## 10. Pacman 默认启动链

```mermaid
flowchart TD
    A["cargo run"] --> B["CHAPTER=pacman"]
    B --> C["build.rs 打包 ch7_pacman"]
    C --> D["kernel loads initproc"]
    D --> E["initproc exec ch7_pacman"]
    E --> F["Game loop"]
    F --> G["read stdin keyboard"]
    F --> H["write fd=3 frame"]
```

## 11. Pacman 图形链

```mermaid
flowchart TD
    A["ch7_pacman Game::submit"] --> B["PacmanFrame"]
    B --> C["write fd=3"]
    C --> D["IO::write"]
    D --> E["graphics::submit_pacman_frame"]
    E --> F["draw map/dots/pacman/ghost"]
    F --> G["VirtIOGpu flush"]
```

## 12. Pacman 输入链

```mermaid
flowchart TD
    A["try_getchar"] --> B["read stdin"]
    B --> C["IO::read fd=0"]
    C --> D["keyboard::take"]
    D --> E["VirtIOInput event"]
    E --> F["keycode -> WASD"]
    F --> G["user buffer"]
```

## 13. 测试链

```text
CHAPTER=-7 cargo run
-> initproc exec ch7b_usertest
-> pipetest / pipe_large_test / signal tests
-> Basic usertests passed!
```

测试脚本强制 headless runner，防止打开 GTK 窗口。

## 14. 已验证

```text
cargo build
CHAPTER=-7 cargo run
```

均已通过。默认 Pacman 会启动图形窗口并持续运行。

