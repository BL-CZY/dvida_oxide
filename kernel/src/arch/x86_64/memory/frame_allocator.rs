use ejcineque::sync::mutex::Mutex;
use once_cell_no_std::OnceCell;
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{FrameAllocator, Page, PageTableFlags, PhysFrame, Size4KiB},
};

use crate::arch::x86_64::memory::{PAGE_SIZE, bitmap::BitMap, page_table::KERNEL_PAGE_TABLE};

use super::memmap;

pub struct MinimalAllocator {
    pub next: usize,
}

unsafe impl FrameAllocator<Size4KiB> for MinimalAllocator {
    fn allocate_frame(&mut self) -> Option<x86_64::structures::paging::PhysFrame<Size4KiB>> {
        let frame = memmap::read_memmap_usable().nth(self.next);
        self.next += 1;
        frame
    }
}

impl MinimalAllocator {
    pub fn step(&mut self, steps: usize) {
        self.next += steps;
    }
}

pub struct BitmapAllocator {
    pub bitmap: BitMap,
    pub next: usize,
}

unsafe impl FrameAllocator<Size4KiB> for BitmapAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let total_bits = self.bitmap.length as usize * 8;

        for offset in 0..total_bits {
            let i = (self.next + offset) % total_bits;
            let byte_idx = i / 8;
            let bit_idx = i % 8;

            if bit_idx == 0 && self.bitmap[byte_idx] == 0xff {
                continue;
            }

            if (self.bitmap[byte_idx] & (1 << bit_idx)) == 0 {
                self.bitmap[byte_idx] |= 1 << bit_idx;

                self.next = (i + 1) % total_bits;

                return unsafe {
                    Some(PhysFrame::from_start_address_unchecked(PhysAddr::new(
                        i as u64 * 4096,
                    )))
                };
            }
        }

        None // Searched everything, no frames left
    }
}

pub static FRAME_ALLOCATOR: OnceCell<Mutex<BitmapAllocator>> = OnceCell::new();

const KERNEL_TASK_STACK_START: u64 = KERNEL_TASK_STACK_GUARD_PAGE + PAGE_SIZE as u64;
const KERNEL_TASK_STACK_GUARD_PAGE: u64 = 0xFFFF_FF00_0000_0000;
const KERNEL_TASK_STACK_LEN: u64 = 16 * PAGE_SIZE as u64;

pub fn setup_stack_for_kernel_task() -> VirtAddr {
    let mut allocator = FRAME_ALLOCATOR
        .get()
        .expect("Failed to get the frame allocator")
        .try_lock()
        .expect("It's not supposed to be locked");

    let mut frames: heapless::Vec<PhysFrame<Size4KiB>, 16> = heapless::Vec::new();

    for _ in 0..15 {
        let frame = allocator
            .allocate_frame()
            .expect("Failed to get physical frame");
        frames.push(frame).expect("Failed to push");
    }

    drop(allocator);

    let kernel_page_table = KERNEL_PAGE_TABLE
        .get()
        .expect("Failed to get kernel page table")
        .try_lock()
        .expect("It's not supposed to be locked");

    for (idx, frame) in frames.iter().enumerate() {
        let page: Page<Size4KiB> = Page::from_start_address(VirtAddr::new(
            KERNEL_TASK_STACK_START + idx as u64 * PAGE_SIZE as u64,
        ))
        .expect("Failed to create page");

        kernel_page_table.map_to(
            page,
            *frame,
            PageTableFlags::NO_EXECUTE | PageTableFlags::WRITABLE | PageTableFlags::PRESENT,
        );
    }

    VirtAddr::new(KERNEL_TASK_STACK_GUARD_PAGE + KERNEL_TASK_STACK_LEN)
}
