use alloc::vec::Vec;
use x86_64::VirtAddr;

use crate::{
    arch::x86_64::{memory::get_hhdm_offset, pcie::PciHeader},
    drivers::ata::sata::AhciSata,
    pcie_offset_impl,
};

#[derive(Debug)]
pub struct Ahci {
    pub location: VirtAddr,
    /// ghc base
    pub base: VirtAddr,
}

impl Ahci {
    // Use these offsets when your struct is pointed at BAR5
    pcie_offset_impl!(
        <cap,      0x00, "r">, // Host Capabilities
        <ghc,      0x04, "rw">, // Global Host Control (AE bit is here!)
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
        let header: PciHeader = unsafe { *(location.as_ptr() as *const PciHeader) };

        let mut phys_base = (header.bars[5] & 0xFFFF_FFF0) as u64;

        let is_64_bit = (header.bars[5] & 0b0100) != 0;

        if is_64_bit {
            let upper_bits = header.bars[4] as u64;
            phys_base |= upper_bits << 32;
        }

        let base = get_hhdm_offset() + phys_base;

        Self { location, base }
    }

    pub fn init(&mut self) -> Vec<AhciSata> {}
}

