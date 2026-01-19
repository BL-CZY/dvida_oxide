use limine::{
    memory_map::{Entry, EntryType},
    request::MemoryMapRequest,
};
use terminal::log;
use x86_64::{PhysAddr, structures::paging::PhysFrame};

use super::get_hhdm_offset;

#[used]
#[unsafe(link_section = ".requests")]
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
            EntryType::FRAMEBUFFER => MemmapEntryType::Framebuffer,
            _ => MemmapEntryType::BadMem,
        }
    }
}

pub fn get_memmap<'a>() -> &'a [&'a Entry] {
    MEMMAP_REQUEST
        .get_response()
        .expect("[Kernel Panic]: Can't get memmap")
        .entries()
}

/// returns (total_memory, total_memory_usable), ignoring the last entry if it's not usable
pub fn sum_memmap(entries: &[&Entry], hhdm_offset: u64, log: bool) -> (u64, u64) {
    let mut total_memory: u64 = 0;
    let mut total_memory_usable: u64 = 0;

    for (index, entry) in entries.iter().enumerate() {
        if log {
            log!(
                "memmap entry {}: type: {:?}, base: {:#x}, length: {:#x}",
                index,
                MemmapEntryType::from_entry(entry.entry_type),
                entry.base + hhdm_offset,
                entry.length
            );
        }

        if MemmapEntryType::from_entry(entry.entry_type) == MemmapEntryType::Usable {
            total_memory_usable += entry.length;
        }

        // ignore the last one if it's not usable
        if MemmapEntryType::from_entry(entry.entry_type) != MemmapEntryType::Usable
            && index >= entries.len() - 1
        {
            if log {
                log!("Ignored the last entry as it's not usable");
            }
            continue;
        }

        total_memory += entry.length;
    }

    (total_memory, total_memory_usable)
}

pub fn log_memmap() {
    let hhdm_offset = get_hhdm_offset().as_u64();

    let (total_memory, total_memory_usable) = sum_memmap(get_memmap(), hhdm_offset, true);

    log!(
        "total memory: {}G (round down), total usable: {}G (round down)",
        total_memory / 0x400 / 0x400 / 0x400,
        total_memory_usable / 0x400 / 0x400 / 0x400
    );
}

pub fn read_memmap_usable() -> impl Iterator<Item = PhysFrame> {
    get_memmap()
        .iter()
        .filter(|r| r.entry_type == EntryType::USABLE)
        .map(move |r| r.base..r.base + r.length)
        .flat_map(|r| r.step_by(super::PAGE_SIZE as usize))
        .map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
}

pub fn count_mem_usable() -> u64 {
    let (_, res) = sum_memmap(get_memmap(), get_hhdm_offset().as_u64(), false);
    res
}

pub fn count_mem() -> u64 {
    let (res, _) = sum_memmap(get_memmap(), get_hhdm_offset().as_u64(), false);
    res
}
