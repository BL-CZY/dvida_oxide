use x86_64::structures::paging::{FrameAllocator, Size4KiB};

use crate::arch::x86_64::memory::bitmap::BitMap;

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
}
