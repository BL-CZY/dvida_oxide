use ejcineque::sync::mutex::Mutex;
use once_cell_no_std::OnceCell;
use x86_64::{
    PhysAddr,
    structures::paging::{FrameAllocator, PhysFrame, Size4KiB},
};

use crate::arch::x86_64::memory::{PAGE_SIZE, bitmap::BitMap};

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
    fn allocate_frame(&mut self) -> Option<x86_64::structures::paging::PhysFrame<Size4KiB>> {
        let mut idx = self.next;

        while idx < self.next + self.bitmap.length as usize * 8 {
            let i = idx % (self.bitmap.length as usize * 8);

            if self.bitmap[i / 8] == 0xff {
                idx = (idx + 8) & !7;
                continue;
            }

            if self.bitmap[i / 8] & 0x1 << (i % 8) == 0 {
                self.bitmap[i / 8] |= 0x1 << (i % 8);
                self.next = i;
                unsafe {
                    return Some(PhysFrame::from_start_address_unchecked(PhysAddr::new(
                        i as u64 * PAGE_SIZE as u64,
                    )));
                }
            }
        }

        None
    }
}

pub static FRAME_ALLOCATOR: OnceCell<Mutex<BitmapAllocator>> = OnceCell::new();
