use core::ops::{Deref, DerefMut};

use limine::memory_map::{Entry, EntryType};
use x86_64::PhysAddr;

use crate::arch::x86_64::memory::{PAGE_SIZE, memmap::get_memmap};

// only the bitmap will use this pointer
unsafe impl Send for BitMap {}
unsafe impl Sync for BitMap {}

pub struct BitMap {
    pub start: *mut u8,
    /// length in bytes
    pub length: u64,
    /// length in pages
    pub page_length: u64,
}

pub fn get_highest_physical_memory_usable() -> u64 {
    let (memmap, len) = get_memmap_length_usable();

    memmap[len - 1].base + memmap[len - 1].length
}

pub fn get_memmap_length_usable<'a>() -> (&'a [&'a Entry], usize) {
    let memmap = get_memmap();

    // ignore all the entires at the end that are not usable
    let mut len = memmap.len();
    for i in memmap.len() - 1..0 {
        if memmap[i].entry_type != EntryType::USABLE {
            len = i;
        } else {
            break;
        }
    }

    (memmap, len)
}

impl Deref for BitMap {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(self.start, self.length as usize) }
    }
}

impl DerefMut for BitMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { core::slice::from_raw_parts_mut(self.start, self.length as usize) }
    }
}

impl BitMap {
    pub fn fill(&self) {
        let (memmap, len) = get_memmap_length_usable();

        let slice = unsafe { core::slice::from_raw_parts_mut(self.start, self.length as usize) };

        // make everything not usable
        for i in slice.iter_mut() {
            *i = !(0);
        }

        for e in memmap[0..len]
            .iter()
            .filter(|e| e.entry_type == EntryType::USABLE)
        {
            self.set_unused_by_address(
                PhysAddr::new(e.base),
                e.length as usize / PAGE_SIZE as usize,
            );
        }
    }

    pub fn set_used_by_address(&self, base: PhysAddr, page_count: usize) {
        let base = base.as_u64();

        let base = base / PAGE_SIZE as u64;

        let slice = unsafe { core::slice::from_raw_parts_mut(self.start, self.length as usize) };

        for i in 0..page_count {
            let idx = base as usize + i;
            slice[idx / 8] = slice[idx / 8] | (0x1 << (idx % 8));
        }
    }

    pub fn set_unused_by_address(&self, base: PhysAddr, page_count: usize) {
        let base = base.as_u64();

        let base = base / PAGE_SIZE as u64;

        let slice = unsafe { core::slice::from_raw_parts_mut(self.start, self.length as usize) };

        for i in 0..page_count {
            let idx = base as usize + i;
            slice[idx / 8] = slice[idx / 8] & !(0x1 << (idx % 8));
        }
    }
}
