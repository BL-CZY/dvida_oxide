use core::ops::DerefMut;

use alloc::format;
use ejcineque::sync::mutex::Mutex;
use limine::request::KernelAddressRequest;
use once_cell_no_std::OnceCell;
use x86_64::{
    VirtAddr,
    registers::control::Cr3,
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame,
        Size4KiB, page_table::PageTableEntry,
    },
};

use crate::arch::x86_64::memory::frame_allocator::FRAME_ALLOCATOR;

use super::get_hhdm_offset;

unsafe impl Send for KernelPageTable {}
unsafe impl Sync for KernelPageTable {}

pub struct KernelPageTable {
    // this is a virtual address
    pub table_ptr: *mut PageTable,
    pub hhdm_offset: VirtAddr,
}

impl KernelPageTable {
    pub fn map_to(&self, page: Page<Size4KiB>, frame: PhysFrame, flags: PageTableFlags) {
        let mut offset_table =
            unsafe { OffsetPageTable::new(&mut (*self.table_ptr), self.hhdm_offset) };

        let mut allocator = FRAME_ALLOCATOR
            .get()
            .expect("Failed to get frame allocator")
            .try_lock()
            .expect("It's not supposed to be locked");

        unsafe {
            offset_table
                .map_to(page, frame, flags, allocator.deref_mut())
                .expect(&format!(
                    "Failed to map frame: {:?} to page {:?} with flags {:?}",
                    frame, page, flags
                ))
                .flush();
        };
    }
}

pub static KERNEL_PAGE_TABLE: OnceCell<Mutex<KernelPageTable>> = OnceCell::new();

pub static KERNEL_ADDRESS_REQUEST: KernelAddressRequest = KernelAddressRequest::new();

pub unsafe fn initialize_page_table() {
    let (table, _) = Cr3::read();

    let phys_addr = table.start_address();
    let virt_addr = get_hhdm_offset() + phys_addr.as_u64();

    let page_table: *mut PageTable = virt_addr.as_mut_ptr();

    let _ = KERNEL_PAGE_TABLE
        .set(Mutex::new(KernelPageTable {
            table_ptr: page_table,
            hhdm_offset: get_hhdm_offset(),
        }))
        .expect("Failed to set kernel page table");
}

pub async fn create_page_table() -> VirtAddr {
    let frame = FRAME_ALLOCATOR
        .get()
        .expect("Failed to get allocator")
        .lock()
        .await
        .allocate_frame()
        .expect("No enough ram");

    let hhdm = get_hhdm_offset();
    let addr = hhdm + frame.start_address().as_u64();
    let target_page_table: &mut PageTable = unsafe { &mut *(addr.as_mut_ptr() as *mut PageTable) };

    let page_table: &mut PageTable = unsafe {
        &mut *(KERNEL_PAGE_TABLE
            .get()
            .expect("Failed to get page table")
            .lock()
            .await
            .table_ptr)
    };

    // copy the higher half

    const PAGE_TABLE_HIGHER_HALF: usize = 256;
    const PAGE_TABLE_ENTRY_COUNT: usize = 512;

    for i in 0..PAGE_TABLE_HIGHER_HALF {
        target_page_table[i] = PageTableEntry::new();
    }

    for i in PAGE_TABLE_HIGHER_HALF..PAGE_TABLE_ENTRY_COUNT {
        target_page_table[i] = page_table[i].clone();
    }

    VirtAddr::from_ptr(target_page_table as *mut PageTable)
}
