use x86_64::VirtAddr;

use crate::{hal::storage::HalBlockDevice, pcie_offset_impl};

pub mod ahci;
pub mod fis;

#[derive(Debug)]
pub struct AhciSata {
    base: VirtAddr,
}

impl HalBlockDevice for AhciSata {
    fn write_sectors_async(
        &mut self,
        index: i64,
        count: u16,
        input: &[u8],
    ) -> core::pin::Pin<
        alloc::boxed::Box<
            dyn Future<Output = Result<(), alloc::boxed::Box<dyn core::error::Error + Send + Sync>>>
                + Send
                + Sync,
        >,
    > {
        todo!()
    }

    fn read_sectors_async(
        &mut self,
        index: i64,
        count: u16,
        output: &mut [u8],
    ) -> core::pin::Pin<
        alloc::boxed::Box<
            dyn Future<Output = Result<(), alloc::boxed::Box<dyn core::error::Error + Send + Sync>>>
                + Send
                + Sync,
        >,
    > {
        todo!()
    }

    fn init(&mut self) -> Result<(), alloc::boxed::Box<dyn core::error::Error + Send + Sync>> {
        todo!()
    }

    fn sector_count(&mut self) -> u64 {
        todo!()
    }

    fn sectors_per_track(&mut self) -> u16 {
        todo!()
    }
}

impl AhciSata {
    pcie_offset_impl!(
        // Command List Base Address (1K aligned)
        <command_list_base_lower, 0x00, "rw">,
        <command_list_base_higher, 0x04, "rw">,

        // FIS Base Address (256B aligned)
        <fis_base_lower, 0x08, "rw">,
        <fis_base_higher, 0x0C, "rw">,

        // Interrupt Status & Enable
        <interrupt_status, 0x10, "rw">,
        <interrupt_enable, 0x14, "rw">,

        // Command and Status
        <command_and_status, 0x18, "rw">,

        // 0x1C is Reserve
        // Task File Data (Status and Error registers from the drive)
        <task_file_data, 0x20, "r">,

        // Signature (Determines if SATA, ATAPI, etc.)
        <signature, 0x24, "r">,

        // SATA Status, Control, and Error (SATA Interface registers)
        <sata_status, 0x28, "r">,
        <sata_control, 0x2C, "rw">,
        <sata_error, 0x30, "rw">,

        // SATA Active (Used for NCQ)
        <sata_active, 0x34, "rw">,

        // Command Issue (Write 1 to bit 'n' to execute command header 'n')
        <command_issue, 0x38, "rw">,

        // SNotification (Used for asynchronous notification)
        <snotification, 0x3C, "rw">,

        // FIS-based Switching Control
        <fbs_control, 0x40, "rw">,

        // 0x44 to 0x6F are Reserved

        // Vendor Specific
        <vendor_specific, 0x70, "rw">
    );
}
