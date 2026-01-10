pub mod bitmap;
pub mod frame_allocator;
pub mod heap;
pub mod memmap;
pub mod page_table;
pub mod pmm;

use crate::arch::x86_64::gdt::{self, AlignedTSS, DOUBLE_FAULT_IST_INDEX, STACK_PAGE_SIZE, TSS};
use crate::arch::x86_64::memory::bitmap::BitMap;
use crate::arch::x86_64::memory::heap::KHeap;
use crate::arch::x86_64::memory::memmap::get_memmap;
use crate::dyn_mem::KHEAP_PAGE_COUNT;
use limine::memory_map::EntryType;
use limine::request::HhdmRequest;
use terminal::{iprintln, log};
use x86_64::VirtAddr;
use x86_64::structures::tss::TaskStateSegment;

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
    let usable_frame_count = memmap::count_mem() / PAGE_SIZE as u64;
    let bitmap_length = (usable_frame_count / BYTE_SIZE as u64)
        + (usable_frame_count % BYTE_SIZE as u64 != 0) as u64;
    let bitmap_page_length =
        (bitmap_length / PAGE_SIZE as u64) + (bitmap_length % PAGE_SIZE as u64 != 0) as u64;

    iprintln!(
        "Usable frame count: {}\nBitmap length: {}\nBitmap page count:{}",
        usable_frame_count,
        bitmap_length,
        bitmap_page_length
    );

    let entry = get_memmap()
        .iter()
        .filter(|r| r.entry_type == EntryType::USABLE)
        .filter(|r| {
            r.length
                > (bitmap_page_length + KHEAP_PAGE_COUNT + STACK_PAGE_SIZE as u64)
                    * PAGE_SIZE as u64
        })
        .next()
        .expect("No Appropriate entry found for kheap, bitmap, and double fault stack");

    let bitmap_start: u64 = entry.base + get_hhdm_offset().as_u64();

    let bit_map = BitMap {
        start: bitmap_start as *mut u8,
        length: bitmap_length,
        page_length: bitmap_page_length,
    };

    let kheap_start: u64 = bitmap_start + bitmap_page_length * PAGE_SIZE as u64;

    let kheap: KHeap = KHeap {
        kheap_start: kheap_start as *mut u8,
    };

    let double_fault_stack_start: u64 = kheap_start
        + KHEAP_PAGE_COUNT * PAGE_SIZE as u64
        + STACK_PAGE_SIZE as u64 * PAGE_SIZE as u64;

    log!(
        "Bitmap at 0x{:x}, Kernel Heap at 0x{:x}, Double Fault Stack at 0x{:x}",
        bitmap_start,
        kheap_start,
        double_fault_stack_start
    );

    let tss = {
        let mut tss = TaskStateSegment::new();
        // tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] =
        //     VirtAddr::from_ptr(double_fault_stack_start as *mut u8);
        tss.interrupt_stack_table[gdt::DOUBLE_FAULT_IST_INDEX as usize] =
            VirtAddr::from_ptr(double_fault_stack_start as *mut u8);
        tss
    };

    let _ = TSS.set(AlignedTSS(tss)).expect("Failed to set tss");

    log!("{:?}", TSS);

    MemoryMappings { bit_map, kheap }
}
