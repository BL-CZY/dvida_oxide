use bitfield::bitfield;
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{Page, PageTableFlags},
};

use crate::{
    arch::x86_64::{
        acpi::MMIO_PAGE_TABLE_FLAGS,
        memory::{
            PAGE_SIZE,
            frame_allocator::FRAME_ALLOCATOR,
            get_hhdm_offset,
            page_table::{KERNEL_PAGE_TABLE, KernelPageTable},
        },
    },
    hal::storage::HalBlockDevice,
    pcie_offset_impl,
    time::get_unix_timestamp,
};

pub mod ahci;
pub mod command;
pub mod fis;

const RECEIVED_FIS_AREA_OFFSET: u64 = 0x500;

bitfield! {
    pub struct PortCmdAndStatus(u32);
    impl Debug;

    // Control Bits
    pub start, set_start: 0;                   // ST: Start processing the command list
    pub spin_up_device, set_spin_up_device: 1; // SUD: Spin-Up Device (for staggered spin-up)
    pub power_on_device, set_power_on_device: 2; // POD: Power On Device
    pub cmd_list_override, set_cmd_list_override: 3; // CLO: Command List Override
    pub fis_recv_enable, set_fis_recv_enable: 4; // FRE: FIS Receive Enable

    // Current State (Read Only)
    pub cur_cmd_slot, _: 12, 8;                // CCS: Current Command Slot being executed

    // Status Bits (Read Only)
    pub mps_state, _: 13;                      // MPSS: Mechanical Presence Switch State
    pub fis_recv_running, _: 14;               // FR: FIS Receive Running
    pub cmd_list_running, _: 15;               // CR: Command List Running
    pub cps_state, _: 16;                      // CPS: Cold Presence State

    // Configuration Bits
    pub port_multiplier_attached, set_port_multiplier_attached: 17; // PMA: Port Multiplier Attached
    pub hot_plug_capable, set_hot_plug_capable: 18; // HPCP: Hot Plug Capable Port
    pub mps_attached, set_mps_attached: 19;     // MPSP: Mechanical Presence Switch Attached
    pub cold_presence_detect, set_cold_presence_detect: 20; // CPD: Cold Presence Detection
    pub external_sata, set_external_sata: 21;   // ESP: External SATA Port
    pub fbcp_supported, set_fbcp_supported: 22; // FBSCP: FIS-based Switching Capable Port
    pub apic_enable, set_apic_enable: 23;       // APSTE: Automatic Partial to Slumber Transitions
    pub atapi_device, set_atapi_device: 24;     // ATAPI: Device is an ATAPI device
    pub drive_led_on_atapi, set_drive_led_on_atapi: 25; // DLAE: Drive LED on ATAPI Enable
    pub aggressive_link_pm, set_aggressive_link_pm: 26; // ALPE: Aggressive Link Power Management Enable
    pub link_pm_state, set_link_pm_state: 27;   // ASP: Aggressive Slumber/Partial (0=Partial, 1=Slumber)

    // Interface State (Read Only)
    pub interface_comm_state, _: 31, 28;       // ICC: Interface Communication Control
}

pub struct TimeOut {}

#[derive(Debug)]
/// each sata will have a buffer
/// the structure of the buffer will be:
/// 0-1023 (0x400) - 32 command headers of 32 bytes each (1kb alignment)
/// 1024-1279 (0x500) - the received fis area (256-byte alignment)
/// 1280-20479 (0x5000) - 32 command tables of 0x200 bytes each
pub struct AhciSata {
    pub base: VirtAddr,
    pub dma_20kb_buffer_vaddr: VirtAddr,
    pub dma_20kb_buffer_paddr: PhysAddr,
    pub max_cmd_slots: u64,
}

impl AhciSata {
    const START: u32 = 0x1 << 0;
    const COMMAND_LIST_RUNNING: u32 = 0x1 << 15;
    const FIS_RECEIVE_ENABLE: u32 = 0x1 << 4;
    const FIS_RECEIVE_RUNNING: u32 = 0x1 << 14;

    pub fn get_buffer(&mut self) -> &mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(
                self.dma_20kb_buffer_vaddr.as_mut_ptr(),
                PAGE_SIZE as usize * 5,
            )
        }
    }

    pub fn new(base: VirtAddr, max_cmd_slots: u64) -> Self {
        let frames = FRAME_ALLOCATOR
            .get()
            .expect("Failed to get allocator")
            .spin_acquire_lock()
            .allocate_continuous_frames(&mut None, 5)
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
                *MMIO_PAGE_TABLE_FLAGS,
            );
        }

        Self {
            base,
            dma_20kb_buffer_vaddr: get_hhdm_offset() + frames[0].start_address().as_u64(),
            dma_20kb_buffer_paddr: frames[0].start_address(),
            max_cmd_slots,
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

    pub fn reset(&mut self) -> Result<(), TimeOut> {
        if self.is_idle() {
            return Ok(());
        }

        let mut cmd_status = self.read_command_and_status();

        cmd_status &= !(Self::START | Self::FIS_RECEIVE_ENABLE);

        self.write_command_and_status(cmd_status);

        let time = get_unix_timestamp();
        loop {
            let cmd_status = self.read_command_and_status();
            let cur = get_unix_timestamp();
            if cur - time > 2 {
                return Err(TimeOut {});
            }

            if cmd_status & (Self::COMMAND_LIST_RUNNING | Self::FIS_RECEIVE_RUNNING) == 0 {
                break;
            }
        }

        Ok(())
    }

    pub fn init(&mut self) -> Result<(), TimeOut> {
        self.reset()?;

        self.write_command_list_base_lower(self.dma_20kb_buffer_paddr.as_u64() as u32);
        self.write_command_list_base_higher((self.dma_20kb_buffer_paddr.as_u64() >> 32) as u32);

        let received_fis_area = self.dma_20kb_buffer_paddr.as_u64() + RECEIVED_FIS_AREA_OFFSET;

        self.write_fis_base_lower(received_fis_area as u32);
        self.write_fis_base_higher((received_fis_area >> 32) as u32);

        self.write_sata_error(0b00000_11111_11111_1_0000_1111_000000_11);

        Ok(())
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
