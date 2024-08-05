use limine::{memory_map::EntryType, request::MemoryMapRequest};

use crate::println;

#[link_section = ".requests"]
static MEMMAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
enum MemmapEntryType {
    Usable = 0,
    Reserved,
    ACPIReclaim,
    ACPINVS,
    BadMem,
    BootReclaim,
    KernalAndMods,
    Framebuffer,
}

impl MemmapEntryType {
    pub fn from_entry(ty: EntryType) -> Self {
        match ty {
            EntryType::USABLE => MemmapEntryType::Usable,
            EntryType::RESERVED => MemmapEntryType::Reserved,
            EntryType::ACPI_RECLAIMABLE => MemmapEntryType::ACPIReclaim,
            EntryType::ACPI_NVS => MemmapEntryType::ACPINVS,
            EntryType::BAD_MEMORY => MemmapEntryType::BadMem,
            EntryType::BOOTLOADER_RECLAIMABLE => MemmapEntryType::BootReclaim,
            EntryType::KERNEL_AND_MODULES => MemmapEntryType::KernalAndMods,
            EntryType::FRAMEBUFFER => MemmapEntryType::Framebuffer,
            _ => MemmapEntryType::BadMem,
        }
    }
}

pub fn init_pmm() {
    let hhdm_offset = if let Some(res) = super::HHDM_REQUEST.get_response() {
        res.offset()
    } else {
        panic!("[Kernal Panic]: No Hhdm offset");
    };

    if let Some(res) = MEMMAP_REQUEST.get_response() {
        for (index, entry) in res.entries().iter().enumerate() {
            println!(
                "memmap entry {}: type: {:?}, base: {:#x}, length: {:#x}",
                index,
                MemmapEntryType::from_entry(entry.entry_type),
                entry.base + hhdm_offset,
                entry.length
            );
        }
    }
}
