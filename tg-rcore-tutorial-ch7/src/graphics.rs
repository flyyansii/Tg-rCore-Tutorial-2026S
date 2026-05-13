//! VirtIO-GPU rendering for the ch7 pacman demo.
#![allow(static_mut_refs)]

use core::ptr::NonNull;

use virtio_drivers::{Hal, MmioTransport, VirtIOGpu, VirtIOHeader};

const VIRTIO_GPU: usize = 0x1000_1000;
const PAGE_SIZE: usize = 4096;
const DMA_PAGES: usize = 1024;
const PACMAN_FRAME_MAGIC: u32 = 0x5041_434D;
const MAP_W: usize = 19;
const MAP_H: usize = 15;
const MAP_SIZE: usize = MAP_W * MAP_H;

/// File descriptor used by the user program to submit pacman frames.
pub const GRAPHICS_FD: usize = 3;

#[repr(C)]
struct PacmanFrame {
    magic: u32,
    tick: u32,
    pac_x: u32,
    pac_y: u32,
    ghost_x: u32,
    ghost_y: u32,
    score: u32,
    lives: u32,
    dots_left: u32,
    game_over: u32,
    win: u32,
    map: [u8; MAP_SIZE],
}

#[repr(align(4096))]
struct DmaMemory {
    bytes: [u8; PAGE_SIZE * DMA_PAGES],
}

static mut DMA_MEMORY: DmaMemory = DmaMemory {
    bytes: [0; PAGE_SIZE * DMA_PAGES],
};
static mut DMA_USED: usize = 0;

struct VirtioHal;

impl Hal for VirtioHal {
    fn dma_alloc(pages: usize) -> usize {
        let size = pages * PAGE_SIZE;
        unsafe {
            let start = (DMA_USED + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
            if start + size > PAGE_SIZE * DMA_PAGES {
                return 0;
            }
            let base = core::ptr::addr_of_mut!(DMA_MEMORY.bytes) as *mut u8;
            core::ptr::write_bytes(base.add(start), 0, size);
            DMA_USED = start + size;
            base.add(start) as usize
        }
    }

    fn dma_dealloc(_paddr: usize, _pages: usize) -> i32 {
        0
    }

    fn phys_to_virt(paddr: usize) -> usize {
        paddr
    }

    fn virt_to_phys(vaddr: usize) -> usize {
        vaddr
    }
}

#[derive(Clone, Copy)]
struct Color {
    r: u8,
    g: u8,
    b: u8,
}

impl Color {
    const BG: Self = Self { r: 3, g: 5, b: 20 };
    const WALL: Self = Self { r: 37, g: 99, b: 235 };
    const DOT: Self = Self { r: 252, g: 231, b: 180 };
    const PAC: Self = Self { r: 250, g: 204, b: 21 };
    const GHOST: Self = Self { r: 244, g: 63, b: 94 };
    const TEXT: Self = Self { r: 226, g: 232, b: 240 };
    const WIN: Self = Self { r: 34, g: 197, b: 94 };
    const OVER: Self = Self { r: 239, g: 68, b: 68 };

    const fn bgra(self) -> u32 {
        self.b as u32 | ((self.g as u32) << 8) | ((self.r as u32) << 16) | 0xff00_0000
    }
}

struct GpuState {
    gpu: VirtIOGpu<'static, VirtioHal, MmioTransport>,
    framebuffer: *mut u8,
    framebuffer_len: usize,
    width: usize,
    height: usize,
}

static mut GPU_STATE: Option<GpuState> = None;

fn log(message: &str) {
    for byte in message.bytes() {
        tg_sbi::console_putchar(byte);
    }
    tg_sbi::console_putchar(b'\n');
}

fn put_str(message: &str) {
    for byte in message.bytes() {
        tg_sbi::console_putchar(byte);
    }
}

fn draw_terminal_number(mut value: usize) {
    let mut buf = [0u8; 20];
    let mut len = 0usize;
    if value == 0 {
        tg_sbi::console_putchar(b'0');
        return;
    }
    while value > 0 {
        buf[len] = b'0' + (value % 10) as u8;
        value /= 10;
        len += 1;
    }
    while len > 0 {
        len -= 1;
        tg_sbi::console_putchar(buf[len]);
    }
}

fn draw_terminal_frame(frame: &PacmanFrame) {
    put_str("\x1b[2J\x1b[H");
    put_str("ch7 pacman demo | WASD/arrows move | Q quit\r\n");
    put_str("score: ");
    draw_terminal_number(frame.score as usize);
    put_str("  lives: ");
    draw_terminal_number(frame.lives as usize);
    put_str("  dots: ");
    draw_terminal_number(frame.dots_left as usize);
    put_str("\r\n");
    let mut y = 0usize;
    while y < MAP_H {
        let mut x = 0usize;
        while x < MAP_W {
            let ch = if frame.pac_x as usize == x && frame.pac_y as usize == y {
                b'C'
            } else if frame.ghost_x as usize == x && frame.ghost_y as usize == y {
                b'G'
            } else {
                match frame.map[y * MAP_W + x] {
                    1 => b'#',
                    2 => b'.',
                    _ => b' ',
                }
            };
            tg_sbi::console_putchar(ch);
            x += 1;
        }
        put_str("\r\n");
        y += 1;
    }
    if frame.win != 0 {
        put_str("YOU WIN\r\n");
    } else if frame.game_over != 0 {
        put_str("GAME OVER\r\n");
    }
}

fn framebuffer_mut(state: &mut GpuState) -> &mut [u8] {
    unsafe { core::slice::from_raw_parts_mut(state.framebuffer, state.framebuffer_len) }
}

fn put_pixel(framebuffer: &mut [u8], width: usize, height: usize, x: usize, y: usize, color: Color) {
    if x >= width || y >= height {
        return;
    }
    let index = (y * width + x) * 4;
    if index + 3 >= framebuffer.len() {
        return;
    }
    unsafe {
        core::ptr::write_unaligned(framebuffer.as_mut_ptr().add(index) as *mut u32, color.bgra());
    }
}

fn fill_rect(
    framebuffer: &mut [u8],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    color: Color,
) {
    let end_y = (y + h).min(height);
    let end_x = (x + w).min(width);
    let pixels = framebuffer.len() / 4;
    let color = color.bgra();
    let mut yy = y;
    while yy < end_y {
        let mut xx = x;
        let row = yy * width;
        while xx < end_x {
            let index = row + xx;
            if index < pixels {
                unsafe {
                    core::ptr::write_unaligned(
                        framebuffer.as_mut_ptr().add(index * 4) as *mut u32,
                        color,
                    );
                }
            }
            xx += 1;
        }
        yy += 1;
    }
}

fn draw_digit(
    framebuffer: &mut [u8],
    width: usize,
    height: usize,
    digit: usize,
    x: usize,
    y: usize,
    scale: usize,
    color: Color,
) {
    const DIGITS: [[u8; 15]; 10] = [
        *b"111101101101111",
        *b"010110010010111",
        *b"111001111100111",
        *b"111001111001111",
        *b"101101111001001",
        *b"111100111001111",
        *b"111100111101111",
        *b"111001001001001",
        *b"111101111101111",
        *b"111101111001111",
    ];
    let pattern = DIGITS[digit % 10];
    let mut row = 0;
    while row < 5 {
        let mut col = 0;
        while col < 3 {
            if pattern[row * 3 + col] == b'1' {
                fill_rect(framebuffer, width, height, x + col * scale, y + row * scale, scale, scale, color);
            }
            col += 1;
        }
        row += 1;
    }
}

fn draw_number(
    framebuffer: &mut [u8],
    width: usize,
    height: usize,
    value: usize,
    x: usize,
    y: usize,
    scale: usize,
    color: Color,
) {
    draw_digit(framebuffer, width, height, value / 100 % 10, x, y, scale, color);
    draw_digit(framebuffer, width, height, value / 10 % 10, x + 4 * scale, y, scale, color);
    draw_digit(framebuffer, width, height, value % 10, x + 8 * scale, y, scale, color);
}

fn draw_circle(
    framebuffer: &mut [u8],
    width: usize,
    height: usize,
    cx: usize,
    cy: usize,
    radius: usize,
    color: Color,
) {
    let r2 = (radius * radius) as isize;
    let start_y = cy.saturating_sub(radius);
    let end_y = (cy + radius).min(height.saturating_sub(1));
    let start_x = cx.saturating_sub(radius);
    let end_x = (cx + radius).min(width.saturating_sub(1));
    let mut y = start_y;
    while y <= end_y {
        let mut x = start_x;
        while x <= end_x {
            let dx = x as isize - cx as isize;
            let dy = y as isize - cy as isize;
            if dx * dx + dy * dy <= r2 {
                put_pixel(framebuffer, width, height, x, y, color);
            }
            x += 1;
        }
        y += 1;
    }
}

fn draw_frame(state: &mut GpuState, frame: &PacmanFrame) {
    let width = state.width;
    let height = state.height;
    let framebuffer = framebuffer_mut(state);
    fill_rect(framebuffer, width, height, 0, 0, width, height, Color::BG);
    draw_number(framebuffer, width, height, frame.score as usize, 36, 18, 5, Color::TEXT);
    draw_number(framebuffer, width, height, frame.lives as usize, width.saturating_sub(126), 18, 5, Color::TEXT);

    let tile = ((width - 96) / MAP_W).min((height - 72) / MAP_H).max(12);
    let board_w = tile * MAP_W;
    let board_x = (width - board_w) / 2;
    let board_y = 54;
    let mut y = 0usize;
    while y < MAP_H {
        let mut x = 0usize;
        while x < MAP_W {
            let px = board_x + x * tile;
            let py = board_y + y * tile;
            match frame.map[y * MAP_W + x] {
                1 => fill_rect(framebuffer, width, height, px + 2, py + 2, tile - 4, tile - 4, Color::WALL),
                2 => draw_circle(framebuffer, width, height, px + tile / 2, py + tile / 2, 3, Color::DOT),
                _ => {}
            }
            x += 1;
        }
        y += 1;
    }

    let pac_cx = board_x + frame.pac_x as usize * tile + tile / 2;
    let pac_cy = board_y + frame.pac_y as usize * tile + tile / 2;
    draw_circle(framebuffer, width, height, pac_cx, pac_cy, tile / 2 - 2, Color::PAC);
    if frame.tick & 2 == 0 {
        fill_rect(framebuffer, width, height, pac_cx, pac_cy.saturating_sub(3), tile / 2, 6, Color::BG);
    }
    let ghost_x = board_x + frame.ghost_x as usize * tile + 3;
    let ghost_y = board_y + frame.ghost_y as usize * tile + 3;
    fill_rect(framebuffer, width, height, ghost_x, ghost_y + tile / 4, tile - 6, tile / 2, Color::GHOST);
    draw_circle(framebuffer, width, height, ghost_x + tile / 2, ghost_y + tile / 3, tile / 2 - 3, Color::GHOST);
    if frame.win != 0 {
        fill_rect(framebuffer, width, height, width / 2 - 130, height / 2 - 24, 260, 48, Color::WIN);
    } else if frame.game_over != 0 {
        fill_rect(framebuffer, width, height, width / 2 - 130, height / 2 - 24, 260, 48, Color::OVER);
    }
}

fn ensure_gpu() -> Option<&'static mut GpuState> {
    unsafe {
        if GPU_STATE.is_none() {
            DMA_USED = 0;
            let transport =
                MmioTransport::new(NonNull::new(VIRTIO_GPU as *mut VirtIOHeader).unwrap()).ok()?;
            let mut gpu = VirtIOGpu::<VirtioHal, MmioTransport>::new(transport).ok()?;
            let (width, height) = gpu.resolution().ok()?;
            let framebuffer = gpu.setup_framebuffer().ok()?;
            let framebuffer_ptr = framebuffer.as_mut_ptr();
            let framebuffer_len = framebuffer.len();
            GPU_STATE = Some(GpuState {
                gpu,
                framebuffer: framebuffer_ptr,
                framebuffer_len,
                width: width as usize,
                height: height as usize,
            });
            log("[ch7-pacman] virtio-gpu ready");
        }
        GPU_STATE.as_mut()
    }
}

/// Draw one pacman frame submitted from user mode.
pub fn looks_like_pacman_frame(buf: usize, count: usize) -> bool {
    if count < core::mem::size_of::<PacmanFrame>() {
        return false;
    }
    unsafe { (*(buf as *const PacmanFrame)).magic == PACMAN_FRAME_MAGIC }
}

/// Draw one pacman frame submitted from user mode.
pub fn submit_pacman_frame(buf: usize, count: usize) -> isize {
    if count < core::mem::size_of::<PacmanFrame>() {
        return -1;
    }
    let frame = unsafe { &*(buf as *const PacmanFrame) };
    if frame.magic != PACMAN_FRAME_MAGIC {
        return -1;
    }
    let Some(state) = ensure_gpu() else {
        log("[ch7-pacman] failed to initialize virtio-gpu");
        draw_terminal_frame(frame);
        return -1;
    };
    draw_frame(state, frame);
    if state.gpu.flush().is_err() {
        return -1;
    }
    count as isize
}

/// Play a deterministic GTK/QEMU graphics demo directly from the kernel.
pub fn run_scripted_demo() -> ! {
    log("[ch7-pacman] kernel gtk demo start");
    let Some(state) = ensure_gpu() else {
        log("[ch7-pacman] failed to initialize virtio-gpu");
        tg_sbi::shutdown(true);
    };
    let mut map = [0u8; MAP_SIZE];
    let raw = [
        b"###################",
        b"#........#........#",
        b"#.###.##.#.##.###.#",
        b"#.................#",
        b"#.##.#.#####.#.##.#",
        b"#....#...#...#....#",
        b"####.### # ###.####",
        b"   #.#       #.#   ",
        b"####.# ## ## #.####",
        b"#........#........#",
        b"#.##.###.#.###.##.#",
        b"#..#...........#..#",
        b"##.#.#.#####.#.#.##",
        b"#....#.......#....#",
        b"###################",
    ];
    let mut y = 0usize;
    while y < MAP_H {
        let mut x = 0usize;
        while x < MAP_W {
            map[y * MAP_W + x] = match raw[y][x] {
                b'#' => 1,
                b'.' => 2,
                _ => 0,
            };
            x += 1;
        }
        y += 1;
    }
    let route = [
        (9, 11), (10, 11), (11, 11), (12, 11), (13, 11), (13, 10), (13, 9), (12, 9),
        (11, 9), (10, 9), (9, 9), (8, 9), (7, 9), (6, 9), (5, 9), (4, 9), (3, 9),
        (2, 9), (1, 9), (1, 10), (1, 11), (2, 11), (3, 11), (4, 11), (5, 11),
    ];
    let mut tick = 0usize;
    while tick < 72 {
        let pac = route[(tick / 5) % route.len()];
        let ghost = route[(tick / 7 + 9) % route.len()];
        let idx = pac.1 * MAP_W + pac.0;
        if map[idx] == 2 {
            map[idx] = 0;
        }
        let frame = PacmanFrame {
            magic: PACMAN_FRAME_MAGIC,
            tick: tick as u32,
            pac_x: pac.0 as u32,
            pac_y: pac.1 as u32,
            ghost_x: ghost.0 as u32,
            ghost_y: ghost.1 as u32,
            score: (tick as u32 / 5) * 10,
            lives: 3,
            dots_left: 120u32.saturating_sub(tick as u32 / 3),
            game_over: 0,
            win: 0,
            map,
        };
        if tick == 0 {
            log("[ch7-pacman] draw first frame");
        }
        draw_frame(state, &frame);
        if tick == 0 {
            log("[ch7-pacman] flush first frame");
        }
        let _ = state.gpu.flush();
        if tick == 0 {
            log("[ch7-pacman] first frame visible");
        }
        delay();
        tick += 1;
    }
    log("[ch7-pacman] Test ch7 pacman OK!");
    tg_sbi::shutdown(false)
}

fn delay() {
    let mut i = 0usize;
    while i < 160_000 {
        core::hint::spin_loop();
        i += 1;
    }
}
