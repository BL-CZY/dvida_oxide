use limine::{
    memory_map::{Entry, EntryType},
    request::MemoryMapRequest,
};
use x86_64::{structures::paging::PhysFrame, PhysAddr};

use crate::println;

use super::HHDM_REQUEST;

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
    KernelAndMods,
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
            EntryType::KERNEL_AND_MODULES => MemmapEntryType::KernelAndMods,
            EntryType::FRAMEBUFFER => MemmapEntryType::Framebuffer,
            _ => MemmapEntryType::BadMem,
        }
    }
}

fn sum_memmap(entries: &[&Entry], hhdm_offset: u64) -> (u64, u64) {
    let mut total_memory: u64 = 0;
    let mut total_memory_usable: u64 = 0;

    for (index, entry) in entries.iter().enumerate() {
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

    (total_memory, total_memory_usable)
}

pub fn log_memmap() {
    let hhdm_offset = if let Some(res) = super::HHDM_REQUEST.get_response() {
        res.offset()
    } else {
        panic!("[Kernel Panic]: No Hhdm offset");
    };

    if let Some(res) = MEMMAP_REQUEST.get_response() {
        let (total_memory, total_memory_usable) = sum_memmap(res.entries(), hhdm_offset);

        // filer out usable entry
        println!(
            "total memory: {}G (round down), total usable: {}G (round down)",
            total_memory / 0x400 / 0x400 / 0x400,
            total_memory_usable / 0x400 / 0x400 / 0x400
        );
    } else {
        panic!("[Kernel Panic]: Can't find memory map");
    }
}

pub fn read_memmap_usable() -> impl Iterator<Item = PhysFrame> {
    if let Some(res) = MEMMAP_REQUEST.get_response() {
        let usable_regions = res
            .entries()
            .iter()
            .filter(|r| r.entry_type == EntryType::USABLE)
            .map(move |r| r.base..r.base + r.length)
            .flat_map(|r| r.step_by(super::PAGE_SIZE as usize))
            .map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)));

        return usable_regions;
    } else {
        panic!("[Kernel Panic]: Can't find memory map");
    }
}
