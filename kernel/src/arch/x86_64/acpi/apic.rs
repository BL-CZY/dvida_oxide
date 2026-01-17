use alloc::vec::Vec;
use bytemuck::{Pod, Zeroable};
use terminal::log;
use x86_64::VirtAddr;

use crate::arch::x86_64::{acpi::AcpiSdtHeader, memory::get_hhdm_offset};

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C, packed)]
pub struct ProcessorLocalApicData {
    /// this is the id used in the acpi context
    pub processor_id: u8,
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
/// highest priority
pub struct LocalApicAddrOverrideData {
    pub reserved: u16,
    pub local_apic_addr: u64,
}

pub struct EntryType {}

impl EntryType {
    const PROCESSOR_LOCAL_APIC: u8 = 0;
    const IO_APIC: u8 = 1;
    const INTERRUPT_SOURCE_OVERRIDE: u8 = 2;
    const NON_MASKABLE_INTERRUPT_SOURCE: u8 = 3;
    const LOCAL_APIC_NONMASKABLE_INTERRUPTS: u8 = 4;
    const LOCAL_APIC_ADDR_OVERRIDE: u8 = 5;
}

#[derive(Pod, Zeroable, Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct MadtEntryHeader {
    pub entry_type: u8,
    pub record_length: u8,
}

#[derive(Pod, Zeroable, Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct MadtHeader {
    pub header: AcpiSdtHeader,
    pub local_apic_addr: u32,
    pub flags: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct ProcessorIds {
    pub processor_id: u8,
    pub apic_id: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct IoApic {
    pub id: u8,
    pub io_apic_addr: VirtAddr,
    // the idx in the idt at which this apic starts
    pub global_system_interrupt_base: u32,
}

#[derive(Debug, Clone, Copy)]
/// those interrupts will fire regardless
pub struct IoNmiSource {
    pub nmi_source: u8,
    pub flags: u16,
    pub global_system_interrupt: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct LocalNmiSource {
    pub acpi_processor_id: u8,
    pub flags: u16,
    pub lint: u8,
}

pub fn discover_apic(mut madt_ptr: VirtAddr) {
    // no need to do the checksum, it's already done
    let header = unsafe { *(madt_ptr.as_ptr() as *const MadtHeader) };
    let mut remaining_length = header.header.length as usize - size_of::<MadtHeader>();
    madt_ptr += size_of::<MadtHeader>() as u64;

    let mut processors: Vec<ProcessorIds> = Vec::new();
    let mut io_apics: Vec<IoApic> = Vec::new();
    let mut isa_io_apic: Option<IoApic> = None;
    let mut local_apic_addr = get_hhdm_offset() + header.local_apic_addr as u64;
    let mut nmi_sources: Vec<IoNmiSource> = Vec::new();
    let mut local_nmi_sources: Vec<LocalNmiSource> = Vec::new();

    let mut isa_irq_gsi: [u32; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

    while remaining_length != 0 {
        let entry_header = unsafe { *(madt_ptr.as_ptr() as *const MadtEntryHeader) };
        madt_ptr += size_of::<MadtEntryHeader>() as u64;

        match entry_header.entry_type {
            EntryType::PROCESSOR_LOCAL_APIC => {
                remaining_length -= entry_header.record_length as usize;

                let data = unsafe { *(madt_ptr.as_ptr() as *const ProcessorLocalApicData) };

                // is it enabled?
                if data.flags & 0b1 == 0b1 {
                    processors.push(ProcessorIds {
                        processor_id: data.processor_id,
                        apic_id: data.apic_id,
                    });
                }
            }

            EntryType::IO_APIC => {
                remaining_length -= entry_header.record_length as usize;

                let data = unsafe { *(madt_ptr.as_ptr() as *const IoApicData) };

                // This is ISA
                if data.global_system_interrupt_base == 0 {
                    isa_io_apic = Some(IoApic {
                        id: data.id,
                        io_apic_addr: get_hhdm_offset() + data.io_apic_addr as u64,
                        global_system_interrupt_base: data.global_system_interrupt_base,
                    });
                }

                io_apics.push(IoApic {
                    id: data.id,
                    io_apic_addr: get_hhdm_offset() + data.io_apic_addr as u64,
                    global_system_interrupt_base: data.global_system_interrupt_base,
                });
            }

            EntryType::INTERRUPT_SOURCE_OVERRIDE => {
                remaining_length -= entry_header.record_length as usize;

                let data =
                    unsafe { *(madt_ptr.as_ptr() as *const IoApicInterruptSourceOverrideData) };

                isa_irq_gsi[data.irq_source as usize] = data.global_system_interrupt;
            }

            EntryType::NON_MASKABLE_INTERRUPT_SOURCE => {
                remaining_length -= entry_header.record_length as usize;

                let data =
                    unsafe { *(madt_ptr.as_ptr() as *const IoApicNonMaskableInterruptSourceData) };

                nmi_sources.push(IoNmiSource {
                    nmi_source: data.nmi_source,
                    flags: data.flags,
                    global_system_interrupt: data.global_system_interrupt,
                });
            }

            EntryType::LOCAL_APIC_NONMASKABLE_INTERRUPTS => {
                remaining_length -= entry_header.record_length as usize;

                let data = unsafe { *(madt_ptr.as_ptr() as *const LocalApicNonMaskableInterrupts) };

                local_nmi_sources.push(LocalNmiSource {
                    acpi_processor_id: data.acpi_processor_id,
                    flags: data.flags,
                    lint: data.lint,
                });
            }

            EntryType::LOCAL_APIC_ADDR_OVERRIDE => {
                remaining_length -= entry_header.record_length as usize;

                let data = unsafe { *(madt_ptr.as_ptr() as *const LocalApicAddrOverrideData) };

                local_apic_addr = get_hhdm_offset() + data.local_apic_addr;
            }

            _ => {
                remaining_length -= entry_header.record_length as usize;
            }
        }

        madt_ptr += entry_header.record_length as u64 - size_of::<MadtEntryHeader>() as u64;
    }

    log!("Processors: {:?}", processors);
    log!("Io Apic(s): {:?}", io_apics);
    log!("ISA Io Apic: {:?}", isa_io_apic.expect("No isa io apic"));
    log!("isa irq gsi mapping : {:?}", isa_irq_gsi);
    log!("Local Apic addr: {:?}", local_apic_addr);
    log!("NMI sources: {:?}", nmi_sources);
    log!("local NMI sources: {:?}", local_nmi_sources);
}
