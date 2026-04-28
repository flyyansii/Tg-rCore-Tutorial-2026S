//! Minimal colored framebuffer demo for ch1-tangram.

use core::ptr::NonNull;

use virtio_drivers::{Hal, MmioTransport, VirtIOGpu, VirtIOHeader};

/// QEMU virt first VirtIO MMIO device base address.
const VIRTIO0: usize = 0x1000_1000;
/// One page is 4 KiB on this simple bare-metal target.
const PAGE_SIZE: usize = 4096;
/// Small static DMA pool used by the VirtIO driver.
const DMA_PAGES: usize = 512;

/// RGB color used by the drawing helpers.
#[derive(Clone, Copy)]
pub struct Color {
    /// Red component.
    pub r: u8,
    /// Green component.
    pub g: u8,
    /// Blue component.
    pub b: u8,
}

#[derive(Clone, Copy)]
struct PolyPoint {
    x: isize,
    y: isize,
}

struct Polygon {
    points: [PolyPoint; 4],
    len: usize,
    color: Color,
}

/// Minimal drawing target abstraction.
pub trait Canvas {
    /// Paint one pixel.
    fn put_pixel(&mut self, x: usize, y: usize, color: Color);
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

struct FramebufferCanvas<'a> {
    framebuffer: &'a mut [u8],
    width: usize,
    height: usize,
}

impl<'a> FramebufferCanvas<'a> {
    fn new(framebuffer: &'a mut [u8], width: usize, height: usize) -> Self {
        Self {
            framebuffer,
            width,
            height,
        }
    }
}

impl Canvas for FramebufferCanvas<'_> {
    fn put_pixel(&mut self, x: usize, y: usize, color: Color) {
        if x >= self.width || y >= self.height {
            return;
        }
        let index = (y * self.width + x) * 4;
        if index + 3 >= self.framebuffer.len() {
            return;
        }
        self.framebuffer[index] = color.b;
        self.framebuffer[index + 1] = color.g;
        self.framebuffer[index + 2] = color.r;
        self.framebuffer[index + 3] = 0xff;
    }
}

fn log(message: &str) {
    for byte in message.bytes() {
        unsafe { (0x1000_0000 as *mut u8).write_volatile(byte) };
    }
    unsafe { (0x1000_0000 as *mut u8).write_volatile(b'\n') };
}

fn edge(a: PolyPoint, b: PolyPoint, p: PolyPoint) -> isize {
    (p.x - a.x) * (b.y - a.y) - (p.y - a.y) * (b.x - a.x)
}

fn contains(poly: &Polygon, p: PolyPoint) -> bool {
    let mut has_pos = false;
    let mut has_neg = false;
    let mut i = 0;
    while i < poly.len {
        let a = poly.points[i];
        let b = poly.points[(i + 1) % poly.len];
        let value = edge(a, b, p);
        if value > 0 {
            has_pos = true;
        }
        if value < 0 {
            has_neg = true;
        }
        i += 1;
    }
    !(has_pos && has_neg)
}

fn draw_polygon<C: Canvas>(canvas: &mut C, poly: Polygon) {
    let mut min_x = poly.points[0].x;
    let mut max_x = poly.points[0].x;
    let mut min_y = poly.points[0].y;
    let mut max_y = poly.points[0].y;
    let mut i = 1;
    while i < poly.len {
        let p = poly.points[i];
        if p.x < min_x {
            min_x = p.x;
        }
        if p.x > max_x {
            max_x = p.x;
        }
        if p.y < min_y {
            min_y = p.y;
        }
        if p.y > max_y {
            max_y = p.y;
        }
        i += 1;
    }

    let mut y = min_y;
    while y <= max_y {
        let mut x = min_x;
        while x <= max_x {
            if x >= 0 && y >= 0 && contains(&poly, PolyPoint { x, y }) {
                canvas.put_pixel(x as usize, y as usize, poly.color);
            }
            x += 1;
        }
        y += 1;
    }
}

fn triangle(a: (isize, isize), b: (isize, isize), c: (isize, isize), color: Color) -> Polygon {
    Polygon {
        points: [
            PolyPoint { x: a.0, y: a.1 },
            PolyPoint { x: b.0, y: b.1 },
            PolyPoint { x: c.0, y: c.1 },
            PolyPoint { x: 0, y: 0 },
        ],
        len: 3,
        color,
    }
}

fn quad(
    a: (isize, isize),
    b: (isize, isize),
    c: (isize, isize),
    d: (isize, isize),
    color: Color,
) -> Polygon {
    Polygon {
        points: [
            PolyPoint { x: a.0, y: a.1 },
            PolyPoint { x: b.0, y: b.1 },
            PolyPoint { x: c.0, y: c.1 },
            PolyPoint { x: d.0, y: d.1 },
        ],
        len: 4,
        color,
    }
}

/// Draw a simple colored "OS" tangram-style logo.
pub fn draw_os_logo<C: Canvas>(canvas: &mut C) {
    let red = Color { r: 220, g: 15, b: 10 };
    let yellow = Color { r: 255, g: 210, b: 20 };
    let cyan = Color { r: 20, g: 190, b: 220 };
    let green = Color { r: 80, g: 240, b: 25 };
    let blue = Color { r: 70, g: 35, b: 235 };
    let magenta = Color { r: 230, g: 70, b: 215 };
    let orange = Color { r: 255, g: 140, b: 0 };

    let pieces = [
        // O, left side.
        triangle((50, 60), (150, 60), (50, 160), red),
        quad((50, 160), (150, 60), (150, 250), (50, 330), yellow),
        triangle((50, 330), (250, 430), (50, 430), cyan),
        quad((150, 350), (250, 250), (350, 350), (250, 430), green),
        quad((250, 150), (350, 250), (350, 350), (250, 250), blue),
        triangle((150, 60), (350, 60), (350, 250), magenta),
        // S, right side. The pieces intentionally overlap a little to avoid gaps.
        triangle((500, 60), (600, 60), (500, 160), cyan),
        triangle((500, 160), (600, 60), (600, 260), cyan),
        triangle((600, 60), (700, 60), (700, 160), blue),
        quad((700, 40), (800, 40), (800, 100), (700, 160), magenta),
        triangle((600, 160), (800, 260), (600, 260), green),
        quad((600, 260), (700, 260), (800, 360), (700, 360), magenta),
        triangle((700, 360), (800, 360), (700, 430), magenta),
        quad((500, 360), (600, 360), (650, 430), (550, 430), orange),
        triangle((600, 360), (700, 360), (650, 430), blue),
    ];

    for piece in pieces {
        draw_polygon(canvas, piece);
    }
}

/// Initialize VirtIO-GPU, draw the logo, and flush it to the screen.
pub fn demo() {
    log("[graphics] init virtio transport");
    let transport = match unsafe {
        MmioTransport::new(NonNull::new(VIRTIO0 as *mut VirtIOHeader).unwrap())
    } {
        Ok(transport) => transport,
        Err(_) => {
            log("[graphics] failed to create virtio transport");
            return;
        }
    };

    log("[graphics] init virtio gpu");
    let mut gpu = match VirtIOGpu::<VirtioHal, MmioTransport>::new(transport) {
        Ok(gpu) => gpu,
        Err(_) => {
            log("[graphics] failed to create virtio gpu");
            return;
        }
    };

    log("[graphics] get resolution");
    let (width, height) = match gpu.resolution() {
        Ok(resolution) => resolution,
        Err(_) => {
            log("[graphics] failed to get gpu resolution");
            return;
        }
    };

    log("[graphics] setup framebuffer");
    let framebuffer = match gpu.setup_framebuffer() {
        Ok(framebuffer) => framebuffer,
        Err(_) => {
            log("[graphics] failed to setup framebuffer");
            return;
        }
    };
    framebuffer.fill(0);

    log("[graphics] draw logo");
    let mut canvas = FramebufferCanvas::new(framebuffer, width as usize, height as usize);
    draw_os_logo(&mut canvas);

    log("[graphics] flush framebuffer");
    if gpu.flush().is_err() {
        log("[graphics] failed to flush framebuffer");
        return;
    }
    log("[graphics] done");
}
