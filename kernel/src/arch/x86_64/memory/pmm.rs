use limine::{memory_map::EntryType, request::MemoryMapRequest};

use crate::println;

#[link_section = ".requests"]
static MEMMAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    let mut total_memory: u64 = 0;
    let mut total_memory_usable: u64 = 0;

    if let Some(res) = MEMMAP_REQUEST.get_response() {
        for (index, entry) in res.entries().iter().enumerate() {
            println!(
                "memmap entry {}: type: {:?}, base: {:#x}, length: {:#x}",
                index,
                MemmapEntryType::from_entry(entry.entry_type),
                entry.base + hhdm_offset,
                entry.length
            );

            if MemmapEntryType::from_entry(entry.entry_type) == MemmapEntryType::Usable {
                total_memory_usable += entry.length;
            }

            total_memory += entry.length;
        }
    }

    println!(
        "total memory: {}G (round down), total usable: {}G (round down)",
        total_memory / 0x400 / 0x400 / 0x400,
        total_memory_usable / 0x400 / 0x400 / 0x400
    );
}
