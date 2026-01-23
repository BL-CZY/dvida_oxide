use alloc::vec::Vec;
use x86_64::{VirtAddr, structures::paging::Page};

use crate::{
    arch::x86_64::{
        acpi::MMIO_PAGE_TABLE_FLAGS,
        idt::{AHCI_INTERRUPT_HANDLER_IDX, CUR_AHCI_INTERRUPT_HANDLER_IDX},
        memory::{PAGE_SIZE, get_hhdm_offset, page_table::KERNEL_PAGE_TABLE},
        msi::{MessageAddressRegister, MessageDataRegister, MsiControl, PcieMsiCapNode},
        pcie::{CapabilityNodeHeader, PciHeader},
    },
    drivers::ata::sata::AhciSata,
    pcie_offset_impl,
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
    /// ghc base
    pub base: VirtAddr,
}

impl AhciHba {
    pcie_offset_impl!(
        <cap,      0x00, "r">, // Host Capabilities
        <ghc,      0x04, "rw">, // Global Host Control
        <is,       0x08, "rw">, // Interrupt Status (Global)
        <pi,       0x0C, "r">, // Ports Implemented (Bitmask)
        <vs,       0x10, "r">, // Version
        <ccc_ctl,  0x14, "rw">, // Command Completion Coalescing Control
        <ccc_pts,  0x18, "rw">, // Command Completion Coalescing Ports
        <em_loc,   0x1C, "r">, // Enclosure Management Location
        <em_ctl,   0x20, "rw">, // Enclosure Management Control
        <cap2,     0x24, "r">, // Host Capabilities Extended
        <bohc,     0x28, "rw">  // BIOS/OS Handoff Control and Status
    );

    pub fn new(location: VirtAddr) -> Self {
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

        page_table.update_flags(
            Page::from_start_address(base.align_down(PAGE_SIZE as u64)).expect("Rust error"),
            *MMIO_PAGE_TABLE_FLAGS,
        );

        Self {
            location,
            header,
            base,
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

        let idx = CUR_AHCI_INTERRUPT_HANDLER_IDX.fetch_add(1, core::sync::atomic::Ordering::AcqRel);
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

        // set GHC.AE
        let mut ghc = self.read_ghc();
        ghc &= !(0x1 << 31);
        ghc |= 0x1 << 31;
        // set GHC.IE
        ghc &= !(0x1 << 1);
        ghc |= 0x1 << 1;

        self.write_ghc(ghc);

        // doesn't support 32 bits only yet
        if self.read_cap() & (0x1 << 31) == 0 {
            return Vec::new();
        }

        // get number of commands from CAP
        let cap = self.read_cap();
        let num_cmd_slots = (cap >> 8) & 0b11111;

        // get devices
        let mut devices: Vec<AhciSata> = Vec::new();
        let pi = self.read_pi();

        for i in 0..32 {
            if pi & 0x1 << i != 0 {
                let mut sata = AhciSata::new(self.base + 0x100 + i * 0x80, num_cmd_slots as u64);

                if sata.init().is_ok() {
                    devices.push(sata);
                }
            }
        }

        devices
    }
}
