pub mod frame_allocator;
pub mod memmap;
pub mod page_table;
pub mod pmm;

use crate::println;
use frame_allocator::MinimalAllocator;
use limine::request::HhdmRequest;
use memmap::read_memmap_usable;
use page_table::read_offset_table;
use x86_64::structures::paging::{Mapper, Page, PageTableFlags};
use x86_64::VirtAddr;

#[link_section = ".requests"]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

pub const PAGE_SIZE: u32 = 4096;

pub fn hhdm_offset() -> VirtAddr {
    VirtAddr::new(
        HHDM_REQUEST
            .get_response()
            .expect("[Kernel Panic]: Can't get HHDM")
            .offset(),
    )
}

pub fn init() {
    // get 16 pages of memory as kernel heap

    let mut page_table = unsafe { read_offset_table() };
    let mut minimal_allocator = MinimalAllocator { next: 0 };

    minimal_allocator.step(16);

    let usable_mem = read_memmap_usable();

    for (index, frame) in usable_mem.enumerate() {
        if index >= 16 {
            break;
        }

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        let page =
            Page::containing_address(VirtAddr::new(0x1000 + PAGE_SIZE as u64 * index as u64));

        unsafe {
            page_table
                .map_to(page, frame, flags, &mut minimal_allocator)
                .expect("Failed to map kernel heap")
                .flush();
        }

        println!("successfully mapped kernel heap to {:?}", page);
    }
}
