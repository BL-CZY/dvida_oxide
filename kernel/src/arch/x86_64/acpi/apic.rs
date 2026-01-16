use alloc::vec::Vec;
use bytemuck::{Pod, Zeroable};

use crate::arch::x86_64::acpi::AcpiSdtHeader;

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C, packed)]
pub struct ProcessorLocalApicData {
    /// this is the id used in the acpi context
    pub processor_id: u8,
    /// this is the physical address of the cpu core
    pub apic_id: u8,
    pub flags: u32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct IoApicData {
    pub id: u8,
    pub reserved: u8,
    pub io_apic_addr: u32,
    // the idx in the idt at which this apic starts
    pub global_system_interrupt_base: u32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
/// only overrides the ISA interrupt sources
pub struct IoApicInterruptSourceOverrideData {
    pub bus_source: u8,
    // the isa index
    pub irq_source: u8,
    // the mapped irq
    pub global_system_interrupt: u32,
    pub flags: u16,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
/// those interrupts will fire regardless
pub struct IoApicNonMaskableInterruptSourceData {
    pub nmi_source: u8,
    pub reserved: u8,
    pub flags: u16,
    pub global_system_interrupt: u32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
/// those are just indicators that the NMIs will be connected to this lint
pub struct LocalApicNonMaskableInterrupts {
    pub acpi_processor_id: u8,
    pub flags: u16,
    pub lint: u8,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
///
pub struct LocalApicAddrOverrideData {
    pub reserved: u16,
    pub local_apic_addr: u64,
}

pub enum ApicData {}

#[repr(u8)]
pub enum EntryType {
    ProcessorLocalApic = 0,
    IoApic = 1,
    InterruptSourceOverride = 2,
    NonMaskableInterruptSource = 3,
    LocalApicNonMaskableInterrupts = 4,
    LocalApicAddrOverride = 5,
    ProcessorLocalx2Apic = 9,
}

#[derive(Pod, Zeroable, Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct ApicEntryHeader {
    pub entry_type: u8,
    pub record_length: u8,
}

pub struct ApicEntry {}

#[derive(Pod, Zeroable, Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct LocalApicData {
    pub local_apic_addr: u32,
    pub flags: u32,
}

pub struct MadtTable {
    pub header: AcpiSdtHeader,
    pub local_apic_data: LocalApicData,
    pub entries: Vec<ApicEntry>,
}

pub fn discover_apic() {}
