//! VirtIO-GPU support for the ch3 snake game.
#![allow(static_mut_refs)]

use core::ptr::NonNull;

use virtio_drivers::{Hal, MmioTransport, VirtIOGpu, VirtIOHeader};

const VIRTIO0: usize = 0x1000_1000;
const PAGE_SIZE: usize = 4096;
const DMA_PAGES: usize = 512;
const MAX_SNAKE: usize = 64;
const SNAKE_FRAME_MAGIC: u32 = 0x534E_4B33;

/// File descriptor used by the user program to submit snake frames.
pub const GRAPHICS_FD: usize = 3;

#[repr(C)]
#[derive(Clone, Copy)]
struct SnakePoint {
    x: u8,
    y: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SnakeFrame {
    magic: u32,
    width: u32,
    height: u32,
    len: u32,
    score: u32,
    food: SnakePoint,
    _padding: [u8; 2],
    snake: [SnakePoint; MAX_SNAKE],
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
    const BLACK: Self = Self { r: 8, g: 10, b: 14 };
    const WALL: Self = Self { r: 40, g: 50, b: 65 };
    const HEAD: Self = Self { r: 255, g: 230, b: 90 };
    const BODY: Self = Self { r: 80, g: 235, b: 90 };
    const FOOD: Self = Self { r: 255, g: 70, b: 90 };
    const TEXT: Self = Self { r: 210, g: 225, b: 240 };
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
    mut value: usize,
    x: usize,
    y: usize,
    scale: usize,
    color: Color,
) {
    let hundreds = value / 100;
    value %= 100;
    let tens = value / 10;
    let ones = value % 10;
    draw_digit(framebuffer, width, height, hundreds, x, y, scale, color);
    draw_digit(framebuffer, width, height, tens, x + 4 * scale, y, scale, color);
    draw_digit(framebuffer, width, height, ones, x + 8 * scale, y, scale, color);
}

fn draw_frame(state: &mut GpuState, frame: &SnakeFrame) {
    let width = state.width;
    let height = state.height;
    let framebuffer = framebuffer_mut(state);
    framebuffer.fill(0);

    let board_w = frame.width as usize;
    let board_h = frame.height as usize;
    if board_w == 0 || board_h == 0 {
        return;
    }
    let cell = ((width - 80) / board_w).min((height - 120) / board_h).max(4);
    let origin_x = (width - board_w * cell) / 2;
    let origin_y = 70;

    fill_rect(framebuffer, width, height, 0, 0, width, height, Color::BLACK);
    fill_rect(
        framebuffer,
        width,
        height,
        origin_x - 8,
        origin_y - 8,
        board_w * cell + 16,
        board_h * cell + 16,
        Color::WALL,
    );
    fill_rect(
        framebuffer,
        width,
        height,
        origin_x,
        origin_y,
        board_w * cell,
        board_h * cell,
        Color::BLACK,
    );

    draw_number(
        framebuffer,
        width,
        height,
        frame.score as usize,
        origin_x,
        24,
        6,
        Color::TEXT,
    );

    let food_x = origin_x + frame.food.x as usize * cell;
    let food_y = origin_y + frame.food.y as usize * cell;
    fill_rect(
        framebuffer,
        width,
        height,
        food_x + cell / 5,
        food_y + cell / 5,
        cell * 3 / 5,
        cell * 3 / 5,
        Color::FOOD,
    );

    let len = (frame.len as usize).min(MAX_SNAKE);
    let mut i = len;
    while i > 0 {
        i -= 1;
        let point = frame.snake[i];
        let x = origin_x + point.x as usize * cell;
        let y = origin_y + point.y as usize * cell;
        let color = if i == 0 { Color::HEAD } else { Color::BODY };
        fill_rect(
            framebuffer,
            width,
            height,
            x + 2,
            y + 2,
            cell.saturating_sub(4),
            cell.saturating_sub(4),
            color,
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
            log("[ch3-snake] virtio-gpu ready");
        }
        GPU_STATE.as_mut()
    }
}

/// Draw one snake frame submitted from user mode.
pub fn submit_snake_frame(buf: usize, count: usize) -> isize {
    if count < core::mem::size_of::<SnakeFrame>() {
        return -1;
    }
    let frame = unsafe { &*(buf as *const SnakeFrame) };
    if frame.magic != SNAKE_FRAME_MAGIC {
        return -1;
    }
    let Some(state) = ensure_gpu() else {
        log("[ch3-snake] failed to initialize virtio-gpu");
        return -1;
    };
    draw_frame(state, frame);
    if state.gpu.flush().is_err() {
        return -1;
    }
    count as isize
}
