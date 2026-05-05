//! VirtIO-GPU rendering for the ch4 Tetris demo.
#![allow(static_mut_refs)]

use core::ptr::NonNull;

use virtio_drivers::{Hal, MmioTransport, VirtIOGpu, VirtIOHeader};

const VIRTIO0: usize = 0x1000_1000;
const PAGE_SIZE: usize = 4096;
const DMA_PAGES: usize = 512;
const BOARD_W: usize = 10;
const BOARD_H: usize = 20;
const TETRIS_FRAME_MAGIC: u32 = 0x5454_5234;

/// File descriptor used by the user program to submit Tetris frames.
pub const GRAPHICS_FD: usize = 3;

#[repr(C)]
struct TetrisFrame {
    magic: u32,
    width: u32,
    height: u32,
    score: u32,
    lines: u32,
    level: u32,
    game_over: u32,
    cells: [u8; BOARD_W * BOARD_H],
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
    const BLACK: Self = Self { r: 7, g: 9, b: 14 };
    const WALL: Self = Self { r: 42, g: 49, b: 66 };
    const GRID: Self = Self { r: 19, g: 24, b: 34 };
    const TEXT: Self = Self { r: 225, g: 232, b: 245 };
    const RED: Self = Self { r: 239, g: 68, b: 68 };
    const ORANGE: Self = Self { r: 249, g: 115, b: 22 };
    const YELLOW: Self = Self { r: 250, g: 204, b: 21 };
    const GREEN: Self = Self { r: 34, g: 197, b: 94 };
    const CYAN: Self = Self { r: 34, g: 211, b: 238 };
    const BLUE: Self = Self { r: 59, g: 130, b: 246 };
    const PURPLE: Self = Self { r: 168, g: 85, b: 247 };
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
        unsafe { (0x1000_0000 as *mut u8).write_volatile(byte) };
    }
    unsafe { (0x1000_0000 as *mut u8).write_volatile(b'\n') };
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
    framebuffer[index] = color.b;
    framebuffer[index + 1] = color.g;
    framebuffer[index + 2] = color.r;
    framebuffer[index + 3] = 0xff;
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
    let mut yy = y;
    while yy < end_y {
        let mut xx = x;
        while xx < end_x {
            put_pixel(framebuffer, width, height, xx, yy, color);
            xx += 1;
        }
        yy += 1;
    }
}

fn color_for(cell: u8) -> Color {
    match cell {
        b'I' => Color::CYAN,
        b'O' => Color::YELLOW,
        b'T' => Color::PURPLE,
        b'S' => Color::GREEN,
        b'Z' => Color::RED,
        b'J' => Color::BLUE,
        b'L' => Color::ORANGE,
        _ => Color::GRID,
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
                fill_rect(
                    framebuffer,
                    width,
                    height,
                    x + col * scale,
                    y + row * scale,
                    scale,
                    scale,
                    color,
                );
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
    mut value: usize,
    x: usize,
    y: usize,
    scale: usize,
    color: Color,
) {
    let mut divisor = 10000;
    let mut started = false;
    let mut offset = 0;
    while divisor > 0 {
        let digit = value / divisor;
        value %= divisor;
        if digit != 0 || started || divisor == 1 {
            started = true;
            draw_digit(framebuffer, width, height, digit, x + offset, y, scale, color);
            offset += 4 * scale;
        }
        divisor /= 10;
    }
}

fn draw_frame(state: &mut GpuState, frame: &TetrisFrame) {
    let width = state.width;
    let height = state.height;
    let framebuffer = framebuffer_mut(state);

    fill_rect(framebuffer, width, height, 0, 0, width, height, Color::BLACK);
    draw_number(
        framebuffer,
        width,
        height,
        frame.score as usize,
        36,
        28,
        5,
        Color::TEXT,
    );
    draw_number(
        framebuffer,
        width,
        height,
        frame.lines as usize,
        36,
        70,
        4,
        Color::GREEN,
    );
    draw_number(
        framebuffer,
        width,
        height,
        frame.level as usize,
        36,
        104,
        4,
        Color::CYAN,
    );

    let cell = ((width - 220) / BOARD_W).min((height - 60) / BOARD_H).max(8);
    let board_w = BOARD_W * cell;
    let board_h = BOARD_H * cell;
    let origin_x = (width - board_w) / 2 + 70;
    let origin_y = (height - board_h) / 2;

    fill_rect(
        framebuffer,
        width,
        height,
        origin_x - 8,
        origin_y - 8,
        board_w + 16,
        board_h + 16,
        Color::WALL,
    );
    fill_rect(
        framebuffer,
        width,
        height,
        origin_x,
        origin_y,
        board_w,
        board_h,
        Color::GRID,
    );

    let mut y = 0;
    while y < BOARD_H {
        let mut x = 0;
        while x < BOARD_W {
            let cell_value = frame.cells[y * BOARD_W + x];
            let color = color_for(cell_value);
            let px = origin_x + x * cell;
            let py = origin_y + y * cell;
            fill_rect(framebuffer, width, height, px + 1, py + 1, cell - 2, cell - 2, color);
            x += 1;
        }
        y += 1;
    }

    if frame.game_over != 0 {
        fill_rect(
            framebuffer,
            width,
            height,
            origin_x + cell,
            origin_y + board_h / 2 - 18,
            board_w - cell * 2,
            36,
            Color::RED,
        );
    }
}

fn ensure_gpu() -> Option<&'static mut GpuState> {
    unsafe {
        if GPU_STATE.is_none() {
            DMA_USED = 0;
            let transport =
                MmioTransport::new(NonNull::new(VIRTIO0 as *mut VirtIOHeader).unwrap()).ok()?;
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
            log("[ch4-tetris] virtio-gpu ready");
        }
        GPU_STATE.as_mut()
    }
}

/// Draw one Tetris frame submitted from user mode.
pub fn submit_tetris_frame(buf: usize, count: usize) -> isize {
    if count < core::mem::size_of::<TetrisFrame>() {
        return -1;
    }
    let frame = unsafe { &*(buf as *const TetrisFrame) };
    if frame.magic != TETRIS_FRAME_MAGIC {
        return -1;
    }
    let Some(state) = ensure_gpu() else {
        log("[ch4-tetris] failed to initialize virtio-gpu");
        return -1;
    };
    draw_frame(state, frame);
    if state.gpu.flush().is_err() {
        return -1;
    }
    count as isize
}
