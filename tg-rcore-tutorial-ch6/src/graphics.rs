//! VirtIO-GPU rendering for the ch6 breakout demo.
#![allow(static_mut_refs)]

use core::ptr::NonNull;

use virtio_drivers::{Hal, MmioTransport, VirtIOGpu, VirtIOHeader};

const VIRTIO_GPU: usize = 0x1000_2000;
const PAGE_SIZE: usize = 4096;
const DMA_PAGES: usize = 256;
const BREAKOUT_FRAME_MAGIC: u32 = 0x4252_4B54;

/// File descriptor used by the user program to submit breakout frames.
pub const GRAPHICS_FD: usize = 3;

#[repr(C)]
struct BreakoutFrame {
    magic: u32,
    width: u32,
    height: u32,
    paddle_x: i32,
    ball_x: i32,
    ball_y: i32,
    bricks: [u8; 60],
    score: u32,
    lives: u32,
    level: u32,
    saved: u32,
    game_over: u32,
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
    const BG: Self = Self { r: 5, g: 8, b: 16 };
    const WALL: Self = Self {
        r: 51,
        g: 65,
        b: 85,
    };
    const PADDLE: Self = Self {
        r: 45,
        g: 212,
        b: 191,
    };
    const BALL: Self = Self {
        r: 250,
        g: 204,
        b: 21,
    };
    const TEXT: Self = Self {
        r: 226,
        g: 232,
        b: 240,
    };
    const SAVE: Self = Self {
        r: 34,
        g: 197,
        b: 94,
    };
    const OVER: Self = Self {
        r: 239,
        g: 68,
        b: 68,
    };
    const BRICKS: [Self; 6] = [
        Self { r: 248, g: 113, b: 113 },
        Self { r: 251, g: 146, b: 60 },
        Self { r: 250, g: 204, b: 21 },
        Self { r: 74, g: 222, b: 128 },
        Self { r: 56, g: 189, b: 248 },
        Self { r: 168, g: 85, b: 247 },
    ];
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
    value: usize,
    x: usize,
    y: usize,
    scale: usize,
    color: Color,
) {
    draw_digit(framebuffer, width, height, value / 100 % 10, x, y, scale, color);
    draw_digit(
        framebuffer,
        width,
        height,
        value / 10 % 10,
        x + 4 * scale,
        y,
        scale,
        color,
    );
    draw_digit(
        framebuffer,
        width,
        height,
        value % 10,
        x + 8 * scale,
        y,
        scale,
        color,
    );
}

fn draw_frame(state: &mut GpuState, frame: &BreakoutFrame) {
    let width = state.width;
    let height = state.height;
    let framebuffer = framebuffer_mut(state);
    fill_rect(framebuffer, width, height, 0, 0, width, height, Color::BG);

    let board_x = 52;
    let board_y = 46;
    let board_w = width.saturating_sub(104).max(260);
    let board_h = height.saturating_sub(72).max(180);
    fill_rect(framebuffer, width, height, board_x, board_y, board_w, 5, Color::WALL);
    fill_rect(framebuffer, width, height, board_x, board_y, 5, board_h, Color::WALL);
    fill_rect(
        framebuffer,
        width,
        height,
        board_x + board_w - 5,
        board_y,
        5,
        board_h,
        Color::WALL,
    );

    draw_number(framebuffer, width, height, frame.score as usize, 72, 14, 5, Color::TEXT);
    draw_number(
        framebuffer,
        width,
        height,
        frame.lives as usize,
        width.saturating_sub(126),
        14,
        5,
        Color::TEXT,
    );
    draw_digit(
        framebuffer,
        width,
        height,
        frame.level as usize,
        width / 2 - 8,
        16,
        4,
        Color::TEXT,
    );

    let brick_cols = 10usize;
    let brick_rows = 6usize;
    let brick_gap = 5usize;
    let brick_w = (board_w - 40 - (brick_cols - 1) * brick_gap) / brick_cols;
    let brick_h = 18usize;
    let brick_x0 = board_x + 20;
    let brick_y0 = board_y + 24;
    let mut row = 0usize;
    while row < brick_rows {
        let mut col = 0usize;
        while col < brick_cols {
            let index = row * brick_cols + col;
            if frame.bricks[index] != 0 {
                fill_rect(
                    framebuffer,
                    width,
                    height,
                    brick_x0 + col * (brick_w + brick_gap),
                    brick_y0 + row * (brick_h + brick_gap),
                    brick_w,
                    brick_h,
                    Color::BRICKS[row % Color::BRICKS.len()],
                );
            }
            col += 1;
        }
        row += 1;
    }

    let paddle_w = 86usize;
    let paddle_h = 12usize;
    let paddle_x = board_x
        + (frame.paddle_x.max(0) as usize).min(board_w.saturating_sub(paddle_w));
    let paddle_y = board_y + board_h - 30;
    fill_rect(
        framebuffer,
        width,
        height,
        paddle_x,
        paddle_y,
        paddle_w,
        paddle_h,
        Color::PADDLE,
    );

    let ball_x = board_x + (frame.ball_x.max(0) as usize).min(board_w.saturating_sub(12));
    let ball_y = board_y + (frame.ball_y.max(0) as usize).min(board_h.saturating_sub(12));
    fill_rect(framebuffer, width, height, ball_x, ball_y, 12, 12, Color::BALL);

    if frame.saved != 0 {
        fill_rect(framebuffer, width, height, width / 2 - 70, height - 30, 140, 12, Color::SAVE);
    }
    if frame.game_over != 0 {
        fill_rect(
            framebuffer,
            width,
            height,
            width / 2 - 120,
            height / 2 - 24,
            240,
            48,
            Color::OVER,
        );
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
            log("[ch6-breakout] virtio-gpu ready");
        }
        GPU_STATE.as_mut()
    }
}

/// Draw one breakout frame submitted from user mode.
pub fn submit_breakout_frame(buf: usize, count: usize) -> isize {
    if count < core::mem::size_of::<BreakoutFrame>() {
        return -1;
    }
    let frame = unsafe { &*(buf as *const BreakoutFrame) };
    if frame.magic != BREAKOUT_FRAME_MAGIC {
        return -1;
    }
    let Some(state) = ensure_gpu() else {
        log("[ch6-breakout] failed to initialize virtio-gpu");
        return -1;
    };
    draw_frame(state, frame);
    if state.gpu.flush().is_err() {
        return -1;
    }
    count as isize
}
