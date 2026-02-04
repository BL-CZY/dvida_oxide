pub mod bitmap;
pub mod frame_allocator;
pub mod heap;
pub mod memmap;
pub mod page_table;
pub mod per_cpu;
pub mod pmm;

use crate::arch::x86_64::gdt::{self, AlignedTSS, STACK_PAGE_SIZE, TSS};
use crate::arch::x86_64::handlers::{RSP0_STACK_GUARD_PAGE, RSP0_STACK_LENGTH};
use crate::arch::x86_64::memory::bitmap::BitMap;
use crate::arch::x86_64::memory::heap::KHeap;
use crate::arch::x86_64::memory::memmap::get_memmap;
use crate::dyn_mem::KHEAP_PAGE_COUNT;
use crate::{iprintln, log};
use limine::memory_map::EntryType;
use limine::mp::Cpu;
use limine::request::HhdmRequest;
use once_cell_no_std::OnceCell;
use x86_64::structures::tss::TaskStateSegment;
use x86_64::{PhysAddr, VirtAddr};

#[used]
#[unsafe(link_section = ".requests")]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

static HHDM_OFFSET: OnceCell<u64> = OnceCell::new();

pub const PAGE_SIZE: u32 = 4096;
pub const PAGE_SIZE_2_MIB: u32 = 4096 * 512;
pub const BYTE_SIZE: u32 = 8;
pub const VIRTMEM_OFFSET: u64 = 0x1000;

pub struct MemoryMappings {
    pub bit_map: BitMap,
    pub kheap: KHeap,
}

pub fn get_hhdm_offset() -> VirtAddr {
    VirtAddr::new(
        *HHDM_OFFSET
            .get_or_init(|| {
                HHDM_REQUEST
                    .get_response()
                    .expect("[Kernel Panic]: Can't get HHDM")
                    .offset()
            })
            .expect("Failed to get hhdm"),
    )
}

pub fn init() -> MemoryMappings {
    let frame_count = bitmap::get_highest_physical_memory_usable() / PAGE_SIZE as u64;
    let bitmap_length = frame_count.div_ceil(BYTE_SIZE as u64);
    let bitmap_page_length = bitmap_length.div_ceil(PAGE_SIZE as u64);

    iprintln!(
        "frame count: {}\nBitmap length: {}\nBitmap page count:{}",
        frame_count,
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

    let hhdm_offset = get_hhdm_offset().as_u64();
    let bitmap_start: u64 = entry.base + hhdm_offset;

    let bit_map = BitMap {
        start: bitmap_start as *mut u8,
        length: bitmap_length,
        page_length: bitmap_page_length,
    };

    let kheap_start: u64 = bitmap_start + bitmap_page_length * PAGE_SIZE as u64;

    let kheap: KHeap = KHeap {
        kheap_start: kheap_start as *mut u8,
    };

    log!(
        "Bitmap at 0x{:x}, Kernel Heap at 0x{:x}",
        bitmap_start,
        kheap_start,
    );

    bit_map.fill();
    bit_map.set_used_by_address(
        PhysAddr::new(bitmap_start - hhdm_offset),
        (bitmap_page_length + KHEAP_PAGE_COUNT) as usize,
    );

    // let tss = {
    //     let mut tss = TaskStateSegment::new();
    //     // tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] =
    //     //     VirtAddr::from_ptr(double_fault_stack_start as *mut u8);
    //     tss.interrupt_stack_table[gdt::PAGE_FAULT_IST_INDEX as usize] =
    //         VirtAddr::from_ptr(page_fault_stack_start as *mut u8);
    //     tss.privilege_stack_table[0] = VirtAddr::new(RSP0_STACK_GUARD_PAGE + RSP0_STACK_LENGTH);
    //     tss
    // };
    //
    // let _ = TSS.set(AlignedTSS(tss)).expect("Failed to set tss");

    MemoryMappings { bit_map, kheap }
}
