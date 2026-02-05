// TODO: support x2apic

use core::sync::atomic::AtomicU64;

use crate::log;
use alloc::{collections::btree_map::BTreeMap, format, string::String, vec::Vec};
use bytemuck::{Pod, Zeroable};
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{Page, PhysFrame, Size4KiB},
};

use crate::arch::x86_64::{
    acpi::{AcpiSdtHeader, MMIO_PAGE_TABLE_FLAGS},
    idt::SPURIOUS_INTERRUPT_HANDLER_IDX,
    memory::{get_hhdm_offset, page_table::KERNEL_PAGE_TABLE},
    pic::PRIMARY_ISA_PIC_OFFSET,
};

pub static LOCAL_APIC_ADDR: AtomicU64 = AtomicU64::new(0);

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
    pub base: VirtAddr,
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
pub struct LocalNmiSourceData {
    pub acpi_processor_id: u8,
    pub flags: u16,
    pub lint: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct LocalApic {
    pub base: VirtAddr,
}

#[derive(Debug, Clone, Copy)]
pub struct Processor {
    pub ids: ProcessorIds,
    pub local_apic: LocalApic,
    pub nmi_source: LocalNmiSource,
}

#[derive(Debug, Clone, Copy)]
pub struct LocalNmiSource {
    pub flags: u16,
    pub lint: u8,
}

impl Processor {
    pub fn new(ids: ProcessorIds, local_apic: LocalApic) -> Self {
        Self {
            ids,
            local_apic,
            nmi_source: LocalNmiSource { flags: 0, lint: 0 },
        }
    }
}

pub fn init_apic(
    mut madt_ptr: VirtAddr,
) -> (BTreeMap<u8, Processor>, [u32; 16], LocalApic, Vec<IoApic>) {
    // no need to do the checksum, it's already done
    let header = unsafe { *(madt_ptr.as_ptr() as *const MadtHeader) };
    let mut remaining_length = header.header.length as usize - size_of::<MadtHeader>();
    madt_ptr += size_of::<MadtHeader>() as u64;

    let mut processors_partial: Vec<ProcessorIds> = Vec::new();
    let mut io_apics: Vec<IoApic> = Vec::new();
    let mut local_apic_addr = get_hhdm_offset() + header.local_apic_addr as u64;
    let mut nmi_sources: Vec<IoNmiSource> = Vec::new();
    let mut local_nmi_sources: Vec<LocalNmiSourceData> = Vec::new();

    let mut isa_irq_gsi: [u32; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
    let mut isa_irq_gsi_trigger_modes_overrides: [Option<u8>; 16] = [None; 16];
    let mut isa_irq_gsi_polarity_overrides: [Option<u8>; 16] = [None; 16];

    let mut processors: BTreeMap<u8, Processor> = BTreeMap::new();

    while remaining_length != 0 {
        let entry_header = unsafe { *(madt_ptr.as_ptr() as *const MadtEntryHeader) };
        madt_ptr += size_of::<MadtEntryHeader>() as u64;

        match entry_header.entry_type {
            EntryType::PROCESSOR_LOCAL_APIC => {
                remaining_length -= entry_header.record_length as usize;

                let data = unsafe { *(madt_ptr.as_ptr() as *const ProcessorLocalApicData) };

                // is it enabled?
                if data.flags & 0b1 == 0b1 {
                    processors_partial.push(ProcessorIds {
                        processor_id: data.processor_id,
                        apic_id: data.apic_id,
                    });
                }
            }

            EntryType::IO_APIC => {
                remaining_length -= entry_header.record_length as usize;

                let data = unsafe { *(madt_ptr.as_ptr() as *const IoApicData) };

                io_apics.push(IoApic {
                    id: data.id,
                    base: get_hhdm_offset() + data.io_apic_addr as u64,
                    global_system_interrupt_base: data.global_system_interrupt_base,
                });
            }

            EntryType::INTERRUPT_SOURCE_OVERRIDE => {
                remaining_length -= entry_header.record_length as usize;

                let data =
                    unsafe { *(madt_ptr.as_ptr() as *const IoApicInterruptSourceOverrideData) };

                const ACTIVE_HIGH: u16 = 0b01;
                const ACTIVE_LOW: u16 = 0b11;

                const EDGE_TRIGGER: u16 = 0b01;
                const LEVEL_TRIGGER: u16 = 0b11;

                if data.flags & 0b11 == ACTIVE_HIGH {
                    isa_irq_gsi_polarity_overrides[data.irq_source as usize] =
                        Some(IoApicInterruptPolarity::HIGH_ACTIVE);
                } else if data.flags & 0b11 == ACTIVE_LOW {
                    isa_irq_gsi_polarity_overrides[data.irq_source as usize] =
                        Some(IoApicInterruptPolarity::LOW_ACTIVE);
                }

                if (data.flags >> 2) & 0b11 == EDGE_TRIGGER {
                    isa_irq_gsi_trigger_modes_overrides[data.irq_source as usize] =
                        Some(IoApicInterruptTriggerMode::EDGE_SENSITIVE);
                } else if (data.flags >> 2) & 0b11 == LEVEL_TRIGGER {
                    isa_irq_gsi_trigger_modes_overrides[data.irq_source as usize] =
                        Some(IoApicInterruptTriggerMode::LEVEL_SENSITIVE);
                }

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

                local_nmi_sources.push(LocalNmiSourceData {
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

    let local_apic = LocalApic {
        base: local_apic_addr,
    };

    for p_ids in processors_partial.drain(0..) {
        processors.insert(p_ids.processor_id, Processor::new(p_ids, local_apic));
    }

    for local_nmi_source in local_nmi_sources.iter() {
        if local_nmi_source.acpi_processor_id == 0xff {
            for (_, val) in processors.iter_mut() {
                val.nmi_source = LocalNmiSource {
                    flags: local_nmi_source.flags,
                    lint: local_nmi_source.lint,
                };
            }
        } else {
            processors
                .entry(local_nmi_source.acpi_processor_id)
                .and_modify(|v| {
                    v.nmi_source = LocalNmiSource {
                        flags: local_nmi_source.flags,
                        lint: local_nmi_source.lint,
                    };
                });
        }
    }

    // map those pages into virtual memory
    let page_table = KERNEL_PAGE_TABLE
        .get()
        .expect("Failed to get page table")
        .spin_acquire_lock();

    page_table.map_to::<Size4KiB>(
        Page::containing_address(local_apic.base),
        PhysFrame::containing_address(PhysAddr::new(local_apic.base - get_hhdm_offset())),
        *MMIO_PAGE_TABLE_FLAGS,
        &mut None,
    );

    let local_apic_id = local_apic.read_id() >> 24;
    log!("Id of the bootstrap cpu: {local_apic_id}");

    processors
        .get_mut(&(local_apic_id as u8))
        .expect("CPU Identity Crises")
        .local_apic
        .enable();

    for io_apic in io_apics.iter_mut() {
        page_table.map_to::<Size4KiB>(
            Page::containing_address(io_apic.base),
            PhysFrame::containing_address(PhysAddr::new(io_apic.base - get_hhdm_offset())),
            *MMIO_PAGE_TABLE_FLAGS,
            &mut None,
        );

        // this is isa
        if io_apic.global_system_interrupt_base == 0 {
            log!("Initializing isa apic: {:?}", io_apic);

            io_apic.isa_bootstrap(
                isa_irq_gsi,
                isa_irq_gsi_trigger_modes_overrides,
                isa_irq_gsi_polarity_overrides,
                local_apic_id as u8,
            );
        }
    }

    LOCAL_APIC_ADDR.store(
        local_apic.base.as_u64(),
        core::sync::atomic::Ordering::Relaxed,
    );

    log!("Processors: {:?}", processors);
    log!("Io Apic(s): {:?}", io_apics);
    log!("isa irq gsi mapping : {:?}", isa_irq_gsi);
    log!("NMI sources: {:?}", nmi_sources);

    (processors, isa_irq_gsi, local_apic, io_apics)
}

#[macro_export]
macro_rules! pcie_offset_impl {
    () => {};

    (<$name:ident, $val:expr, "r", $tp:ty>, $($rest:tt)*) => {
        $crate::pcie_port_readonly!($name, $tp, |self| { (self.base + $val).as_mut_ptr() });
        $crate::pcie_offset_impl!($($rest)*);
    };

    (<$name:ident, $val:expr, "w", $tp:ty>, $($rest:tt)*) => {
        $crate::pcie_port_writeonly!($name, $tp, |self| { (self.base + $val).as_mut_ptr() });
        $crate::pcie_offset_impl!($($rest)*);
    };

    (<$name:ident, $val:expr, "rw", $tp:ty>, $($rest:tt)*) => {
        $crate::pcie_port_readwrite!($name, $tp, |self| { (self.base + $val).as_mut_ptr() });
        $crate::pcie_offset_impl!($($rest)*);
    };

    (<$name:ident, $val:expr, "r">, $($rest:tt)*) => {
        $crate::pcie_port_readonly!($name, u32, |self| { (self.base + $val).as_mut_ptr() });
        $crate::pcie_offset_impl!($($rest)*);
    };

    (<$name:ident, $val:expr, "w">, $($rest:tt)*) => {
        $crate::pcie_port_writeonly!($name, u32, |self| { (self.base + $val).as_mut_ptr() });
        $crate::pcie_offset_impl!($($rest)*);
    };

    (<$name:ident, $val:expr, "rw">, $($rest:tt)*) => {
        $crate::pcie_port_readwrite!($name, u32, |self| { (self.base + $val).as_mut_ptr() });
        $crate::pcie_offset_impl!($($rest)*);
    };


    (<$name:ident, $val:expr, "r">) => {
        $crate::pcie_offset_impl!(<$name, $val, "r">, );
    };

    (<$name:ident, $val:expr, "w">) => {
        $crate::pcie_offset_impl!(<$name, $val, "w">, );
    };

    (<$name:ident, $val:expr, "rw">) => {
        $crate::pcie_offset_impl!(<$name, $val, "rw">, );
    };

    (<$name:ident, $val:expr, "r", $tp:ty>) => {
        $crate::pcie_offset_impl!(<$name, $val, "r", $tp>, );
    };

    (<$name:ident, $val:expr, "w", $tp:ty>) => {
        $crate::pcie_offset_impl!(<$name, $val, "w", $tp>, );
    };

    (<$name:ident, $val:expr, "rw", $tp:ty>) => {
        $crate::pcie_offset_impl!(<$name, $val, "rw", $tp>, );
    };
}

impl LocalApic {
    pub fn dump(&self) -> String {
        let mut s = String::new();
        s.push_str("--- Local APIC Dump ---\n");

        // Basic Info
        s.push_str(&format!(
            "ID:            {:#010x} (Shifted: {})\n",
            self.read_id(),
            self.read_id() >> 24
        ));
        s.push_str(&format!("Version:       {:#010x}\n", self.read_version()));
        s.push_str(&format!(
            "Spurious Vec:  {:#010x}\n",
            self.read_spurious_interrupt_vector()
        ));

        // Priorities
        s.push_str(&format!(
            "Task Priority: {:#010x}\n",
            self.read_task_priority()
        ));
        s.push_str(&format!(
            "Proc Priority: {:#010x}\n",
            self.read_processor_priority()
        ));

        // Timer
        s.push_str(&format!("Timer LVT:     {:#010x}\n", self.read_lvt_timer()));
        s.push_str(&format!(
            "Timer Init:    {:#010x}\n",
            self.read_timer_initial_count()
        ));
        s.push_str(&format!(
            "Timer Current: {:#010x}\n",
            self.read_timer_current_count()
        ));
        s.push_str(&format!(
            "Timer Divide:  {:#010x}\n",
            self.read_timer_divide_config()
        ));

        // ICR
        s.push_str(&format!("ICR (High):    {:#010x}\n", self.read_icr_high()));
        s.push_str(&format!("ICR (Low):     {:#010x}\n", self.read_icr_low()));

        // Error
        s.push_str(&format!(
            "Error Status:  {:#010x}\n",
            self.read_error_status()
        ));

        s.push_str("-----------------------\n");
        s
    }

    pcie_offset_impl!(
        <id, 0x20, "r">,
        <version, 0x30, "r">,
        <task_priority, 0x80, "rw">,
        <arbitration_priority, 0x90, "r">,
        <processor_priority, 0xA0, "r">,
        <eoi, 0xB0, "w">,
        <remote_read, 0xC0, "r">,
        <logical_destination, 0xD0, "rw">,
        <destination_format, 0xE0, "rw">,
        <spurious_interrupt_vector, 0xF0, "rw">,

        <error_status, 0x280, "r">,
        <lvt_cmci, 0x2F0, "rw">,

        // Interrupt Command Register (Split into two 32-bit halves)
        <icr_low, 0x300, "rw">,
        <icr_high, 0x310, "rw">,

        // Local Vector Table (LVT)
        <lvt_timer, 0x320, "rw">,
        <lvt_thermal, 0x330, "rw">,
        <lvt_perf_mon, 0x340, "rw">,
        <lvt_lint0, 0x350, "rw">,
        <lvt_lint1, 0x360, "rw">,
        <lvt_error, 0x370, "rw">,

        // Timer Registers
        <timer_initial_count, 0x380, "rw">,
        <timer_current_count, 0x390, "r">,
        <timer_divide_config, 0x3E0, "rw">
    );

    pub fn read_isr(&self, number: u64) -> u32 {
        const ISR_BASE: u64 = 0x100;
        const ALIGNMENT: u64 = 0x10;
        let addr = self.base + ISR_BASE + number * ALIGNMENT;
        let addr: *const u32 = addr.as_ptr();
        unsafe { addr.read_volatile() }
    }

    pub fn read_tmr(&self, number: u64) -> u32 {
        const TMR_BASE: u64 = 0x180;
        const ALIGNMENT: u64 = 0x10;
        let addr = self.base + TMR_BASE + number * ALIGNMENT;
        let addr: *const u32 = addr.as_ptr();
        unsafe { addr.read_volatile() }
    }

    pub fn read_irr(&self, number: u64) -> u32 {
        const IRR_BASE: u64 = 0x200;
        const ALIGNMENT: u64 = 0x10;
        let addr = self.base + IRR_BASE + number * ALIGNMENT;
        let addr: *const u32 = addr.as_ptr();
        unsafe { addr.read_volatile() }
    }

    pub fn enable(&mut self) {
        self.write_task_priority(0);
        self.write_spurious_interrupt_vector((SPURIOUS_INTERRUPT_HANDLER_IDX as u32) | (0x1 << 8));
    }
}

pub struct IoApicDeliveryMode {}
impl IoApicDeliveryMode {
    pub const FIXED: u8 = 0b000;
    pub const LOWEST_PRIORITY: u8 = 0b001;
    pub const SMI: u8 = 0b010;
    pub const NMI: u8 = 0b100;
    pub const INIT: u8 = 0b101;
    pub const EXT_INT: u8 = 0b111;
}

pub struct IoApicInterruptMask {}
impl IoApicInterruptMask {
    pub const MASKED: u8 = 1;
    pub const UNMASKED: u8 = 0;
}

pub struct IoApicInterruptPolarity {}
impl IoApicInterruptPolarity {
    pub const HIGH_ACTIVE: u8 = 0;
    pub const LOW_ACTIVE: u8 = 1;
}

pub struct IoApicInterruptTriggerMode {}
impl IoApicInterruptTriggerMode {
    pub const EDGE_SENSITIVE: u8 = 0;
    pub const LEVEL_SENSITIVE: u8 = 1;
}

pub struct IoApicDestinationMode {}
impl IoApicDestinationMode {
    pub const PHYSICAL: u8 = 0;
    pub const LOGICAL: u8 = 1;
}

pub struct IoApicRedirectionEntry(pub u64);

impl IoApicRedirectionEntry {
    pub fn set_vector(&mut self, vector: u8) {
        self.0 = (self.0 & !0b11111111) + vector as u64;
    }

    pub fn get_vector(&self) -> u8 {
        (self.0 & 0b11111111) as u8
    }

    pub fn get_delivery_mode(&self) -> u8 {
        ((self.0 >> 8) & 0b111) as u8
    }

    pub fn set_delivery_mode(&mut self, mode: u8) {
        self.0 &= !(0b111u64 << 8);
        self.0 |= (mode as u64) << 8;
    }

    pub fn get_destination_mode(&self) -> u8 {
        ((self.0 >> 11) & 0b1) as u8
    }

    pub fn set_destination_mode(&mut self, mode: u8) {
        self.0 &= !(0b1u64 << 11);
        self.0 |= (mode as u64) << 11;
    }

    pub fn get_delivery_status(&self) -> u8 {
        ((self.0 >> 12) & 0b1) as u8
    }

    pub fn get_polarity(&self) -> u8 {
        ((self.0 >> 13) & 0b1) as u8
    }

    pub fn set_polarity(&mut self, polarity: u8) {
        self.0 &= !(0b1u64 << 13);
        self.0 |= (polarity as u64) << 13;
    }

    pub fn get_remote_irr(&self) -> u8 {
        ((self.0 >> 14) & 0b1) as u8
    }

    pub fn get_trigger_mode(&self) -> u8 {
        ((self.0 >> 15) & 0b1) as u8
    }

    pub fn set_trigger_mode(&mut self, mode: u8) {
        self.0 &= !(0b1u64 << 15);
        self.0 |= (mode as u64) << 15;
    }

    pub fn get_interrupt_mask(&self) -> u8 {
        ((self.0 >> 16) & 0b1) as u8
    }

    pub fn set_interrupt_mask(&mut self, mask: u8) {
        self.0 &= !(0b1u64 << 16);
        self.0 |= (mask as u64) << 16;
    }

    pub fn get_destination(&self) -> u8 {
        (self.0 >> 56) as u8
    }

    pub fn set_destination(&mut self, destination: u8) {
        self.0 &= !(0b11111111u64 << 56);
        self.0 |= (destination as u64) << 56;
    }
}

impl IoApic {
    pcie_offset_impl!(<io_cmd, 0x00, "rw">, <io_data, 0x10, "rw">);

    const IOAPICID: u32 = 0x00;
    const IOAPICVER: u32 = 0x01;
    const IOAPICARB: u32 = 0x02;
    const REDIRECTION_TABLE_BASE: u32 = 0x10;

    pub fn read_id(&mut self) -> u32 {
        self.write_io_cmd(Self::IOAPICID);
        self.read_io_data()
    }

    pub fn write_id(&mut self, input: u32) {
        self.write_io_cmd(Self::IOAPICID);
        self.write_io_cmd(input);
    }

    pub fn read_version(&mut self) -> u32 {
        self.write_io_cmd(Self::IOAPICVER);
        self.read_io_data()
    }

    pub fn read_arbitration_id(&mut self) -> u32 {
        self.write_io_cmd(Self::IOAPICARB);
        self.read_io_data()
    }

    pub fn write_redirection_entry(&mut self, entry_num: u8, value: u64) {
        let register_id = Self::REDIRECTION_TABLE_BASE + entry_num as u32 * 2;
        self.write_io_cmd(register_id);
        self.write_io_data(value as u32);
        self.write_io_cmd(register_id + 1);
        self.write_io_data((value >> 32) as u32);
    }

    pub fn read_redirection_entry(&mut self, entry_num: u8) -> u64 {
        let register_id = Self::REDIRECTION_TABLE_BASE + entry_num as u32 * 2;
        let mut res;
        self.write_io_cmd(register_id);
        res = self.read_io_data() as u64;
        self.write_io_cmd(register_id + 1);
        res += (self.read_io_data() as u64) << 32;
        res
    }

    pub fn isa_bootstrap(
        &mut self,
        irq_to_gsi_map: [u32; 16],
        isa_irq_gsi_trigger_modes_overrides: [Option<u8>; 16],
        isa_irq_gsi_polarity_overrides: [Option<u8>; 16],
        local_apic_id: u8,
    ) {
        for i in 0..16 {
            let gsi = irq_to_gsi_map[i as usize];

            // Skip if this GSI belongs to a different I/O APIC
            if gsi < self.global_system_interrupt_base
                || gsi > self.global_system_interrupt_base + ((self.read_version() >> 16) & 0xFF)
            {
                continue;
            }

            let idx_in_apic = gsi - self.global_system_interrupt_base;
            let mut entry = IoApicRedirectionEntry(0);

            entry.set_vector(PRIMARY_ISA_PIC_OFFSET + i);
            entry.set_delivery_mode(IoApicDeliveryMode::FIXED);
            entry.set_destination_mode(IoApicDestinationMode::PHYSICAL);

            if let Some(p) = isa_irq_gsi_polarity_overrides[i as usize] {
                entry.set_polarity(p);
            } else {
                entry.set_polarity(IoApicInterruptPolarity::HIGH_ACTIVE);
            }

            if let Some(m) = isa_irq_gsi_trigger_modes_overrides[i as usize] {
                entry.set_trigger_mode(m);
            } else {
                entry.set_trigger_mode(IoApicInterruptTriggerMode::EDGE_SENSITIVE);
            }

            // no pit interrupts
            if i != 0 {
                entry.set_interrupt_mask(IoApicInterruptMask::UNMASKED);
            } else {
                entry.set_interrupt_mask(IoApicInterruptMask::MASKED);
            }

            entry.set_destination(local_apic_id);

            self.write_redirection_entry(idx_in_apic as u8, entry.0);
        }
    }
}

pub fn get_local_apic() -> LocalApic {
    LocalApic {
        base: VirtAddr::new(LOCAL_APIC_ADDR.load(core::sync::atomic::Ordering::Relaxed)),
    }
}
