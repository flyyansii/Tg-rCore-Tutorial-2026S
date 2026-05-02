//! VirtIO keyboard input for the ch3 snake game.
#![allow(static_mut_refs)]

use core::ptr::NonNull;

use core::sync::atomic::{AtomicUsize, Ordering};
use virtio_drivers::{Hal, MmioTransport, VirtIOHeader, VirtIOInput};

const VIRTIO_KEYBOARD: usize = 0x1000_2000;
const PAGE_SIZE: usize = 4096;
const DMA_PAGES: usize = 64;
const EV_KEY: u16 = 1;
const KEY_Q: u16 = 16;
const KEY_W: u16 = 17;
const KEY_A: u16 = 30;
const KEY_S: u16 = 31;
const KEY_D: u16 = 32;

#[repr(align(4096))]
struct DmaMemory {
    bytes: [u8; PAGE_SIZE * DMA_PAGES],
}

static mut DMA_MEMORY: DmaMemory = DmaMemory {
    bytes: [0; PAGE_SIZE * DMA_PAGES],
};
static mut DMA_USED: usize = 0;
static mut KEYBOARD: Option<VirtIOInput<VirtioHal, MmioTransport>> = None;
static LAST_KEY: AtomicUsize = AtomicUsize::new(0);

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

fn log(message: &str) {
    for byte in message.bytes() {
        unsafe { (0x1000_0000 as *mut u8).write_volatile(byte) };
    }
    unsafe { (0x1000_0000 as *mut u8).write_volatile(b'\n') };
}

fn keycode_to_ascii(code: u16) -> Option<u8> {
    match code {
        KEY_W => Some(b'w'),
        KEY_A => Some(b'a'),
        KEY_S => Some(b's'),
        KEY_D => Some(b'd'),
        KEY_Q => Some(b'q'),
        _ => None,
    }
}

fn ensure_keyboard() -> Option<&'static mut VirtIOInput<VirtioHal, MmioTransport>> {
    unsafe {
        if KEYBOARD.is_none() {
            let transport =
                MmioTransport::new(NonNull::new(VIRTIO_KEYBOARD as *mut VirtIOHeader).unwrap())
                    .ok()?;
            KEYBOARD = Some(VirtIOInput::<VirtioHal, MmioTransport>::new(transport).ok()?);
            log("[ch3-snake] virtio-keyboard ready");
        }
        KEYBOARD.as_mut()
    }
}

/// Poll pending keyboard events and remember the latest supported key.
pub fn refresh() {
    let Some(keyboard) = ensure_keyboard() else {
        return;
    };
    while let Some(event) = keyboard.pop_pending_event() {
        if event.event_type == EV_KEY && event.value != 0 {
            if let Some(byte) = keycode_to_ascii(event.code) {
                LAST_KEY.store(byte as usize + 1, Ordering::Relaxed);
            }
        }
    }
}

/// Take one translated key from the virtio keyboard buffer.
pub fn take() -> Option<u8> {
    refresh();
    let val = LAST_KEY.swap(0, Ordering::Relaxed);
    if val == 0 {
        None
    } else {
        Some((val - 1) as u8)
    }
}
