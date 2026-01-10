use limine::request::KernelAddressRequest;
use terminal::log;
use x86_64::{
    registers::control::Cr3,
    structures::paging::{FrameAllocator, OffsetPageTable, PageTable, Size4KiB},
};

use super::get_hhdm_offset;

pub static KERNEL_ADDRESS_REQUEST: KernelAddressRequest = KernelAddressRequest::new();

pub unsafe fn read_offset_table() -> OffsetPageTable<'static> {
    let l4_tbl = unsafe { read_active_table() };
    unsafe { OffsetPageTable::new(l4_tbl, get_hhdm_offset()) }
}

/// should only be called once
pub unsafe fn read_active_table() -> &'static mut PageTable {
    let (table, _) = Cr3::read();

    let phys_addr = table.start_address();
    let virt_addr = get_hhdm_offset() + phys_addr.as_u64();

    let page_table: *mut PageTable = virt_addr.as_mut_ptr();

    unsafe { &mut *page_table }
}

pub fn create_page_table(allocator: &mut impl FrameAllocator<Size4KiB>, hhdm_offset: u64) {
    let l4_table_frame = allocator
        .allocate_frame()
        .expect("Failed to initialize the level 4 page table");

    let l4_table_ptr = (l4_table_frame.start_address().as_u64() + hhdm_offset) as *mut u8;

    let l4_table_buf =
        unsafe { core::slice::from_raw_parts_mut(l4_table_ptr, l4_table_frame.size() as usize) };

    l4_table_buf.fill(0);

    let kernel_address_response = KERNEL_ADDRESS_REQUEST
        .get_response()
        .expect("No Kernel Address acquired");

    log!(
        "Kernel Physical base: 0x{:x}",
        kernel_address_response.physical_base()
    );

    log!(
        "Kernel Virtual base: 0x{:x}",
        kernel_address_response.virtual_base(),
    );
}
