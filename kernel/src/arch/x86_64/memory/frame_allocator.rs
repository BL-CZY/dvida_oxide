use alloc::vec::Vec;
use ejcineque::sync::mutex::Mutex;
use once_cell_no_std::OnceCell;
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{FrameAllocator, Page, PageTableFlags, PhysFrame, Size4KiB},
};

use crate::arch::x86_64::memory::{PAGE_SIZE, bitmap::BitMap, page_table::KERNEL_PAGE_TABLE};

pub struct BitmapAllocator {
    pub bitmap: BitMap,
    pub next: usize,
}

impl BitmapAllocator {
    pub fn free_frames(&mut self, frames: &[PhysFrame]) {
        for frame in frames.iter() {
            let idx = frame.start_address().as_u64() / PAGE_SIZE as u64;
            let idx = idx as usize;

            self.bitmap[idx / 8] = self.bitmap[idx / 8] & !(0x1 << (idx % 8));
        }
    }
}

unsafe impl FrameAllocator<Size4KiB, Option<Vec<PhysFrame<Size4KiB>>>> for BitmapAllocator {
    /// must be used along with set_current_allocating_thread
    fn allocate_frame(
        &mut self,
        context: &mut Option<Vec<PhysFrame<Size4KiB>>>,
    ) -> Option<PhysFrame<Size4KiB>> {
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
                unsafe {
                    let res: PhysFrame<Size4KiB> =
                        PhysFrame::from_start_address_unchecked(PhysAddr::new(i as u64 * 4096));

                    if let Some(v) = context {
                        v.push(res.clone());
                    }

                    return Some(res);
                };
            }
        }

        None // Searched everything, no frames left
    }
}

pub static FRAME_ALLOCATOR: OnceCell<Mutex<BitmapAllocator>> = OnceCell::new();

pub fn setup_stack(guard_page_loc: u64, len: u64) -> VirtAddr {
    let stack_start: u64 = guard_page_loc + PAGE_SIZE as u64;

    let mut allocator = FRAME_ALLOCATOR
        .get()
        .expect("Failed to get the frame allocator")
        .try_lock()
        .expect("It's not supposed to be locked");

    let mut frames: heapless::Vec<PhysFrame<Size4KiB>, 16> = heapless::Vec::new();

    for _ in 0..15 {
        let frame = allocator
            .allocate_frame(&mut None)
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
        let page: Page<Size4KiB> =
            Page::from_start_address(VirtAddr::new(stack_start + idx as u64 * PAGE_SIZE as u64))
                .expect("Failed to create page");

        kernel_page_table.map_to(
            page,
            *frame,
            PageTableFlags::NO_EXECUTE
                | PageTableFlags::WRITABLE
                | PageTableFlags::PRESENT
                | PageTableFlags::GLOBAL,
        );
    }

    VirtAddr::new(guard_page_loc + len)
}

pub fn setup_stack_for_kernel_task() -> VirtAddr {
    const KERNEL_TASK_STACK_GUARD_PAGE: u64 = 0xFFFF_FF80_0000_0000;
    const KERNEL_TASK_STACK_LEN: u64 = 16 * PAGE_SIZE as u64;

    setup_stack(KERNEL_TASK_STACK_GUARD_PAGE, KERNEL_TASK_STACK_LEN)
}
