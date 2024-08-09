use x86_64::{
    registers::control::Cr3,
    structures::paging::{OffsetPageTable, PageTable},
};

use super::hhdm_offset;

pub unsafe fn read_offset_table() -> OffsetPageTable<'static> {
    let l4_tbl = unsafe { read_active_table() };
    unsafe { OffsetPageTable::new(l4_tbl, hhdm_offset()) }
}

/// should only be called once
pub unsafe fn read_active_table() -> &'static mut PageTable {
    let (table, _) = Cr3::read();

    let phys_addr = table.start_address();
    let virt_addr = hhdm_offset() + phys_addr.as_u64();

    let page_table: *mut PageTable = virt_addr.as_mut_ptr();

    &mut *page_table
}
