use alloc::vec::Vec;
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{Page, PhysFrame, Size4KiB},
};

use crate::{
    arch::x86_64::{
        acpi::MMIO_PAGE_TABLE_FLAGS,
        idt::AHCI_INTERRUPT_HANDLER_IDX,
        memory::{get_hhdm_offset, page_table::KERNEL_PAGE_TABLE},
        msi::{MessageAddressRegister, MessageDataRegister, MsiControl, PcieMsiCapNode},
        pcie::{CapabilityNodeHeader, PciHeader},
    },
    drivers::ata::sata::{AhciSata, task::AHCI_PORTS_MAP},
    log, pcie_offset_impl,
};

#[derive(Debug)]
/// the AHCI HBA device is a device following the AHCI standard regarding communicating to SATA
/// drivers
/// Its global base is stored at BAR[5]
/// It supports multiple SATA drives
/// The CAP's bit structure:
/// 0-4 - number of ports, max 32
/// 8-12 - number of command slots, max 32
///
/// The GHC's bit structure:
/// 0 - rw - hardware reset
/// 1 - rw - interupt enable
/// 2 - ro - MSI Revert to Single Message (MRSM)
/// 31 - rw - set to enable AHCI
pub struct AhciHba {
    pub location: VirtAddr,
    pub header: PciHeader,
    pub ports: AhciHbaPorts,
    pub idx: usize,
}

#[derive(Debug)]
pub struct AhciHbaPorts {
    /// ghc base
    pub base: VirtAddr,
}

impl AhciHbaPorts {
    pcie_offset_impl!(
        <cap,      0x00, "r">, // Host Capabilities
        <ghc,      0x04, "rw">, // Global Host Control
        <interrupt_status,       0x08, "rw">, // Interrupt Status (Global)
        <pi,       0x0C, "r">, // Ports Implemented (Bitmask)
        <vs,       0x10, "r">, // Version
        <ccc_ctl,  0x14, "rw">, // Command Completion Coalescing Control
        <ccc_pts,  0x18, "rw">, // Command Completion Coalescing Ports
        <em_loc,   0x1C, "r">, // Enclosure Management Location
        <em_ctl,   0x20, "rw">, // Enclosure Management Control
        <cap2,     0x24, "r">, // Host Capabilities Extended
        <bohc,     0x28, "rw">  // BIOS/OS Handoff Control and Status
    );
}

pub const HBA_PORT_PORTS_OFFSET: u64 = 0x100;
pub const HBA_PORT_SIZE: u64 = 0x80;

impl AhciHba {
    pub fn new(location: VirtAddr, hba_idx: usize) -> Self {
        // the BAR address *can* be 64 bits so we use the mask to check, if it's 64 bits bars[4]
        // will be used as the higher half
        let header: PciHeader = PciHeader { base: location };

        let mut phys_base = (header.read_bar5() & 0xFFFF_FFF0) as u64;

        let is_64_bit = (header.read_bar5() & 0b0100) != 0;

        if is_64_bit {
            let upper_bits = header.read_bar4() as u64;
            phys_base |= upper_bits << 32;
        }

        let base = get_hhdm_offset() + phys_base;

        let page_table = KERNEL_PAGE_TABLE
            .get()
            .expect("Failed to get page table")
            .spin_acquire_lock();

        page_table.map_to::<Size4KiB>(
            Page::containing_address(base),
            PhysFrame::containing_address(PhysAddr::new(phys_base)),
            *MMIO_PAGE_TABLE_FLAGS,
            &mut None,
        );

        let _ = AHCI_PORTS_MAP[hba_idx].set(location);

        log!("created new ahci");

        Self {
            location,
            header,
            ports: AhciHbaPorts { base },
            idx: hba_idx,
        }
    }

    pub fn init(&mut self) -> Vec<AhciSata> {
        const CAPABILITY_BIT: u16 = 0x1 << 4;
        if self.header.read_status() & CAPABILITY_BIT == 0 {
            return Vec::new();
        }

        let ptr = self.header.read_capabilities_ptr();
        let ptr = self.location + ptr as u64;

        let mut cap_node_header: CapabilityNodeHeader = unsafe { *(ptr.as_ptr()) };

        let mut msi_cap_node = loop {
            if cap_node_header.cap_id == CapabilityNodeHeader::MSI {
                break PcieMsiCapNode { base: ptr };
            }

            if cap_node_header.next == 0 {
                return Vec::new();
            }

            let ptr = self.location + cap_node_header.next as u64;

            cap_node_header = unsafe { *(ptr.as_ptr()) };
        };

        let control_reg = MsiControl(msi_cap_node.read_message_control_register());

        let idx = AHCI_INTERRUPT_HANDLER_IDX + self.idx as u8;
        let mut msi_data = MessageDataRegister::default();
        msi_data.set_vector(idx as u32);
        let msi_addr = MessageAddressRegister::default();

        msi_cap_node.write_message_addr_register(msi_addr.0);

        if control_reg.address_64() {
            msi_cap_node.write_message_upper_addr_register(0);
            msi_cap_node.write_message_data_register_64_bit(msi_data.0);
        } else {
            msi_cap_node.write_message_data_register(msi_data.0);
        }

        log!("Configured Interrupts of AHCI");

        // set GHC.AE
        let mut ghc = self.ports.read_ghc();
        ghc &= !(0x1 << 31);
        ghc |= 0x1 << 31;
        // set GHC.IE
        ghc &= !(0x1 << 1);
        ghc |= 0x1 << 1;

        self.ports.write_ghc(ghc);

        // doesn't support 32 bits only yet
        if self.ports.read_cap() & (0x1 << 31) == 0 {
            return Vec::new();
        }

        // get number of commands from CAP
        let cap = self.ports.read_cap();
        let num_cmd_slots = 1 + ((cap >> 8) & 0b11111);

        log!("Num cmd slots: {}", num_cmd_slots);

        // get devices
        let mut devices: Vec<AhciSata> = Vec::new();
        let pi = self.ports.read_pi();

        for i in 0..32 {
            if pi & 0x1 << i != 0 {
                let mut sata = if let Some(s) = AhciSata::new(
                    self.ports.base + HBA_PORT_PORTS_OFFSET + i * HBA_PORT_SIZE,
                    num_cmd_slots as u64,
                    self.idx,
                    i as usize,
                ) {
                    s
                } else {
                    continue;
                };

                if sata.init().is_ok() {
                    log!("Creating new sata");
                    devices.push(sata);
                }
            }
        }

        devices
    }
}
