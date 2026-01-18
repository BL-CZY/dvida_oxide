use core::ops::DerefMut;

use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{Page, PageTableFlags},
};

use crate::{
    arch::x86_64::memory::{
        PAGE_SIZE,
        frame_allocator::FRAME_ALLOCATOR,
        get_hhdm_offset,
        page_table::{KERNEL_PAGE_TABLE, KernelPageTable},
    },
    hal::storage::HalBlockDevice,
    pcie_offset_impl,
};

pub mod ahci;
pub mod fis;

#[derive(Debug)]
pub struct AhciSata {
    pub base: VirtAddr,
    pub dma_16kb_buffer_vaddr: VirtAddr,
    pub dma_16kb_buffer_paddr: PhysAddr,
}

impl AhciSata {
    const START: u32 = 0x1 << 0;
    const COMMAND_LIST_RUNNING: u32 = 0x1 << 15;
    const FIS_RECEIVE_ENABLE: u32 = 0x1 << 4;
    const FIS_RECEIVE_RUNNING: u32 = 0x1 << 14;

    pub fn new(base: VirtAddr) -> Self {
        let frames = FRAME_ALLOCATOR
            .get()
            .expect("Failed to get allocator")
            .spin_acquire_lock()
            .allocate_continuous_frames(&mut None, 4)
            .expect("No enough memory");

        let page_table = KERNEL_PAGE_TABLE
            .get()
            .expect("Failed to get page table")
            .spin_acquire_lock();

        for frame in frames.iter() {
            unsafe {
                core::slice::from_raw_parts_mut(
                    (get_hhdm_offset() + frame.start_address().as_u64()).as_mut_ptr::<u8>(),
                    PAGE_SIZE as usize,
                )
                .fill(0);
            }

            page_table.update_flags(
                Page::from_start_address(get_hhdm_offset() + frame.start_address().as_u64())
                    .expect("Frame allocator corrupted"),
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE,
            );
        }

        Self {
            base,
            dma_16kb_buffer_vaddr: get_hhdm_offset() + frames[0].start_address().as_u64(),
            dma_16kb_buffer_paddr: frames[0].start_address(),
        }
    }

    pub fn is_idle(&mut self) -> bool {
        let cmd_status = self.read_command_and_status();

        if cmd_status
            & (Self::START
                | Self::COMMAND_LIST_RUNNING
                | Self::FIS_RECEIVE_ENABLE
                | Self::FIS_RECEIVE_RUNNING)
            != 0
        {
            false
        } else {
            true
        }
    }

    pub fn reset(&mut self) {
        if self.is_idle() {
            return;
        }

        let mut cmd_status = self.read_command_and_status();

        cmd_status &= !(Self::START | Self::FIS_RECEIVE_ENABLE);

        self.write_command_and_status(cmd_status);

        loop {
            let cmd_status = self.read_command_and_status();

            if cmd_status & (Self::COMMAND_LIST_RUNNING | Self::FIS_RECEIVE_RUNNING) == 0 {
                break;
            }
        }
    }
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
