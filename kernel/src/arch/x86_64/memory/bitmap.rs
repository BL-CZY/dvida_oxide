use limine::memory_map::EntryType;
use x86_64::VirtAddr;

use crate::arch::x86_64::memory::{PAGE_SIZE, get_hhdm_offset, memmap::get_memmap};

pub struct BitMap {
    pub start: *mut u8,
    /// length in bytes
    pub length: u64,
    /// length in pages
    pub page_length: u64,
}

impl BitMap {
    pub fn fill(&self) {
        let memmap = get_memmap();

        let all_bits = memmap
            .iter()
            .enumerate()
            .filter(|(idx, e)| {
                if *idx == memmap.len() - 1 && e.entry_type != EntryType::USABLE {
                    false
                } else {
                    true
                }
            })
            .map(|(_, e)| e)
            .map(|e| (e.entry_type == EntryType::USABLE, e))
            .map(move |(usable, e)| (usable, e.base..e.base + e.length))
            .flat_map(|(usable, r)| r.step_by(PAGE_SIZE as usize).map(move |_| usable));

        let slice = unsafe { core::slice::from_raw_parts_mut(self.start, self.length as usize) };

        for i in slice.iter_mut() {
            *i = 0;
        }

        // the first page is not usable
        slice[0] = 0x1;

        for (idx, is_usable) in all_bits.enumerate() {
            let idx = idx + 1;

            if is_usable {
                slice[idx / 8] = slice[idx / 8] & !(0x1 << (idx % 8));
            } else {
                slice[idx / 8] = slice[idx / 8] | (0x1 << (idx % 8));
            }
        }
    }

    pub fn set_used_by_address(&self, base: VirtAddr, page_count: usize) {
        let base = base.as_u64();
        let hhdm = get_hhdm_offset().as_u64();
        let base = base - hhdm;

        let base = base / PAGE_SIZE as u64;

        let slice = unsafe { core::slice::from_raw_parts_mut(self.start, self.length as usize) };

        for i in 0..page_count {
            let idx = base as usize + i;
            slice[idx / 8] = slice[idx / 8] | (0x1 << (idx % 8));
        }
    }
}
