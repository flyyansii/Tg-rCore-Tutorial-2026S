//! VirtIO-GPU rendering for the ch5 pingpong demo.
#![allow(static_mut_refs)]

use core::ptr::NonNull;

use virtio_drivers::{Hal, MmioTransport, VirtIOGpu, VirtIOHeader};

const VIRTIO0: usize = 0x1000_1000;
const PAGE_SIZE: usize = 4096;
const DMA_PAGES: usize = 256;
const PINGPONG_FRAME_MAGIC: u32 = 0x504F_4E47;

/// File descriptor used by the user program to submit pingpong frames.
pub const GRAPHICS_FD: usize = 3;

#[repr(C)]
struct PingpongFrame {
    magic: u32,
    width: u32,
    height: u32,
    left_y: i32,
    right_y: i32,
    ball_x: i32,
    ball_y: i32,
    left_score: u32,
    right_score: u32,
    speed: u32,
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
    const BLACK: Self = Self { r: 4, g: 7, b: 12 };
    const COURT: Self = Self { r: 10, g: 18, b: 32 };
    const LINE: Self = Self {
        r: 90,
        g: 105,
        b: 130,
    };
    const LEFT: Self = Self {
        r: 34,
        g: 211,
        b: 238,
    };
    const RIGHT: Self = Self {
        r: 248,
        g: 113,
        b: 113,
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
    const WARN: Self = Self {
        r: 249,
        g: 115,
        b: 22,
    };
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
    draw_digit(framebuffer, width, height, value / 10 % 10, x, y, scale, color);
    draw_digit(
        framebuffer,
        width,
        height,
        value % 10,
        x + 4 * scale,
        y,
        scale,
        color,
    );
}

fn clamp_to_screen(value: i32, max: usize) -> usize {
    if value < 0 {
        0
    } else {
        (value as usize).min(max)
    }
}

fn draw_frame(state: &mut GpuState, frame: &PingpongFrame) {
    let width = state.width;
    let height = state.height;
    let framebuffer = framebuffer_mut(state);
    let court_x = 60;
    let court_y = 42;
    let court_w = width.saturating_sub(120).max(200);
    let court_h = height.saturating_sub(86).max(160);

    fill_rect(framebuffer, width, height, 0, 0, width, height, Color::BLACK);
    fill_rect(
        framebuffer,
        width,
        height,
        court_x,
        court_y,
        court_w,
        court_h,
        Color::COURT,
    );
    fill_rect(framebuffer, width, height, court_x, court_y, court_w, 4, Color::LINE);
    fill_rect(
        framebuffer,
        width,
        height,
        court_x,
        court_y + court_h - 4,
        court_w,
        4,
        Color::LINE,
    );

    let mut y = court_y + 10;
    while y + 10 < court_y + court_h {
        fill_rect(framebuffer, width, height, court_x + court_w / 2 - 2, y, 4, 14, Color::LINE);
        y += 28;
    }

    draw_number(
        framebuffer,
        width,
        height,
        frame.left_score as usize,
        court_x + court_w / 2 - 92,
        12,
        7,
        Color::LEFT,
    );
    draw_number(
        framebuffer,
        width,
        height,
        frame.right_score as usize,
        court_x + court_w / 2 + 34,
        12,
        7,
        Color::RIGHT,
    );
    draw_number(
        framebuffer,
        width,
        height,
        frame.speed as usize,
        court_x + court_w - 70,
        18,
        3,
        Color::TEXT,
    );

    let paddle_h = court_h / 5;
    let paddle_w = 12;
    let left_y = court_y + clamp_to_screen(frame.left_y, court_h.saturating_sub(paddle_h));
    let right_y = court_y + clamp_to_screen(frame.right_y, court_h.saturating_sub(paddle_h));
    fill_rect(framebuffer, width, height, court_x + 18, left_y, paddle_w, paddle_h, Color::LEFT);
    fill_rect(
        framebuffer,
        width,
        height,
        court_x + court_w - 30,
        right_y,
        paddle_w,
        paddle_h,
        Color::RIGHT,
    );

    let ball_x = court_x + clamp_to_screen(frame.ball_x, court_w.saturating_sub(14));
    let ball_y = court_y + clamp_to_screen(frame.ball_y, court_h.saturating_sub(14));
    fill_rect(framebuffer, width, height, ball_x, ball_y, 14, 14, Color::BALL);

    if frame.game_over != 0 {
        fill_rect(
            framebuffer,
            width,
            height,
            court_x + court_w / 2 - 120,
            court_y + court_h / 2 - 24,
            240,
            48,
            Color::WARN,
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
            log("[ch5-pingpong] virtio-gpu ready");
        }
        GPU_STATE.as_mut()
    }
}

/// Draw one pingpong frame submitted from user mode.
pub fn submit_pingpong_frame(buf: usize, count: usize) -> isize {
    if count < core::mem::size_of::<PingpongFrame>() {
        return -1;
    }
    let frame = unsafe { &*(buf as *const PingpongFrame) };
    if frame.magic != PINGPONG_FRAME_MAGIC {
        return -1;
    }
    let Some(state) = ensure_gpu() else {
        log("[ch5-pingpong] failed to initialize virtio-gpu");
        return -1;
    };
    draw_frame(state, frame);
    if state.gpu.flush().is_err() {
        return -1;
    }
    count as isize
}
