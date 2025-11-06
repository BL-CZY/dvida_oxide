pub mod bitmap;
pub mod frame_allocator;
pub mod heap;
pub mod memmap;
pub mod page_table;
pub mod pmm;

use crate::arch::x86_64::memory::bitmap::BitMap;
use crate::arch::x86_64::memory::heap::KHeap;
use crate::dyn_mem::KHEAP_PAGE_COUNT;
use crate::{print, println};
use frame_allocator::MinimalAllocator;
use limine::request::HhdmRequest;
use memmap::read_memmap_usable;
use page_table::read_offset_table;
use x86_64::VirtAddr;
use x86_64::structures::paging::{Mapper, Page, PageTableFlags};

#[unsafe(link_section = ".requests")]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

pub const PAGE_SIZE: u32 = 4096;
pub const BYTE_SIZE: u32 = 8;
pub const VIRTMEM_OFFSET: u64 = 0x1000;

pub struct MemoryMappings {
    pub bit_map: BitMap,
    pub kheap: KHeap,
}

pub fn get_hhdm_offset() -> VirtAddr {
    VirtAddr::new(
        HHDM_REQUEST
            .get_response()
            .expect("[Kernel Panic]: Can't get HHDM")
            .offset(),
    )
}

pub fn init() -> MemoryMappings {
    // get 16 pages of memory as kernel heap

    let mut page_table = unsafe { read_offset_table() };
    let mut minimal_allocator = MinimalAllocator { next: 0 };

    let usable_frame_count = memmap::count_mem_usable() / PAGE_SIZE as u64;
    let bitmap_length = (usable_frame_count / BYTE_SIZE as u64)
        + (usable_frame_count % BYTE_SIZE as u64 != 0) as u64;
    let bitmap_page_length =
        (bitmap_length / PAGE_SIZE as u64) + (bitmap_length % PAGE_SIZE as u64 != 0) as u64;

    println!(
        "Usable frame count: {}\nBitmap length: {}\nBitmap page count:{}",
        usable_frame_count, bitmap_length, bitmap_page_length
    );

    // reserve a space for bitmap and kernel heap so we don't write page tables there
    minimal_allocator.step(bitmap_page_length as usize + KHEAP_PAGE_COUNT as usize);

    let usable_mem = read_memmap_usable();

    let bitmap_start: u64 = VIRTMEM_OFFSET;

    let bit_map = BitMap {
        start: bitmap_start as *mut u8,
        length: bitmap_length,
        page_length: bitmap_page_length,
    };

    let kheap_start: u64 = VIRTMEM_OFFSET + bitmap_page_length;

    let kheap: KHeap = KHeap {
        kheap_start: kheap_start as *mut u8,
    };

    println!("[Mapped pages (b for bitmap, k for kernel heap)]:");

    for (index, frame) in usable_mem.enumerate() {
        if index >= (KHEAP_PAGE_COUNT + bitmap_page_length) as usize {
            break;
        }

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        let page = Page::containing_address(VirtAddr::new(
            VIRTMEM_OFFSET + PAGE_SIZE as u64 * index as u64,
        ));

        unsafe {
            page_table
                .map_to(page, frame, flags, &mut minimal_allocator)
                .expect("Failed to map kernel heap")
                .flush();
        }

        if index < bitmap_page_length as usize {
            print!("b{:?}, ", page);
        } else {
            print!("k{:?}, ", page);
        }
    }

    MemoryMappings { bit_map, kheap }
}
