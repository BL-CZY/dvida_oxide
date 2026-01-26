use core::time::Duration;

use alloc::{boxed::Box, vec};
use bitfield::bitfield;
use x86_64::{PhysAddr, VirtAddr, structures::paging::Page};

use crate::{
    arch::x86_64::{
        acpi::MMIO_PAGE_TABLE_FLAGS,
        memory::{
            PAGE_SIZE, frame_allocator::FRAME_ALLOCATOR, get_hhdm_offset,
            page_table::KERNEL_PAGE_TABLE,
        },
        timer::Instant,
    },
    drivers::ata::sata::{
        command::{
            CommandHeader, CommandHeaderFlags, CommandTable, IdentifyData, PrdtEntry,
            PrdtEntryFlags,
        },
        fis::{AtaCommand, FisRegH2DFlags},
    },
    ejcineque::sync::mpsc::unbounded::UnboundedReceiver,
    hal::storage::{HalBlockDevice, HalStorageOperation, SECTOR_SIZE},
    log, pcie_offset_impl,
};

pub mod ahci;
pub mod command;
pub mod fis;
pub mod task;

const RECEIVED_FIS_AREA_OFFSET: u64 = 0x400;
const CMD_TABLES_OFFSET: u64 = 0x500;
const CMD_TABLE_SIZE: u64 = 0x200;

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

    pub interface_comm_control, set_interface_comm_control: 31, 28;       // ICC: Interface Communication Control
}

pub struct TimeOut {}

#[derive(Debug)]
/// each sata will have a buffer
/// the structure of the buffer will be:
/// 0-1023 (0x400) - 32 command headers of 32 bytes each (1kb alignment)
/// 1024-1279 (0x500) - the received fis area (256-byte alignment)
/// 1280-20479 (0x5000) - 32 command tables of 0x200 bytes each
pub struct AhciSata {
    pub ports: AhciSataPorts,
    pub dma_20kb_buffer_vaddr: VirtAddr,
    pub dma_20kb_buffer_paddr: PhysAddr,
    pub max_cmd_slots: u64,
    pub sectors_per_track: u16,
    pub sector_count: u64,
    pub hba_idx: usize,
    pub ports_idx: usize,
}

bitfield! {
    pub struct PortStatus(u32);
    impl Debug;

    pub interface_power_management, _: 11, 8;
    pub current_interface_speed, _: 7, 4;
    pub device_detection, _: 3, 0;
}

impl PortStatus {
    pub const DET_NOT_PRESENT: u32 = 0x0;
    pub const DET_PRESENT_NO_PHY: u32 = 0x1;
    pub const DET_PRESENT_WITH_PHY: u32 = 0x3;
    pub const DET_OFFLINE: u32 = 0x4;

    pub const IPM_NOT_PRESENT: u32 = 0x0;
    pub const IPM_ACTIVE: u32 = 0x1;
    pub const IPM_PARTIAL: u32 = 0x2;
    pub const IPM_SLUMBER: u32 = 0x6;
    pub const IPM_DEVSLEEP: u32 = 0x8;

    pub const SPD_NOT_PRESENT: u32 = 0x0;
    pub const SPD_GEN1_1_5GBPS: u32 = 0x1;
    pub const SPD_GEN2_3_0GBPS: u32 = 0x2;
    pub const SPD_GEN3_6_0GBPS: u32 = 0x3;
}

bitfield! {
    pub struct PortControl(u32);
    impl Debug;
    pub ipm_restrictions, set_ipm_restrictions: 11, 8;

    pub speed_allowed, set_speed_allowed: 7, 4;

    pub det_init, set_det_init: 3, 0;
}

impl PortControl {
    pub const DET_NO_ACTION: u32 = 0x0;
    pub const DET_COMRESET: u32 = 0x1;
    pub const DET_DISABLE_PHY: u32 = 0x4;

    pub const SPD_NO_LIMIT: u32 = 0x0;
    pub const SPD_LIMIT_GEN1_1P5: u32 = 0x1;
    pub const SPD_LIMIT_GEN2_3P0: u32 = 0x2;
    pub const SPD_LIMIT_GEN3_6P0: u32 = 0x3;

    pub const IPM_NO_RESTRICTIONS: u32 = 0x0;
    pub const IPM_DISABLE_PARTIAL: u32 = 0x1;
    pub const IPM_DISABLE_SLUMBER: u32 = 0x2;
    pub const IPM_DISABLE_BOTH: u32 = 0x3;
}

bitfield! {
    pub struct PortInterruptEnable(u32);
    impl Debug;
    pub cold_presence_detect_enable, set_cold_presence_detect_enable: 31;
    pub task_file_error_enable, set_task_file_error_enable: 30;
    pub host_bus_fatal_error_enable, set_host_bus_fatal_error_enable: 29;
    pub host_bus_data_error_enable, set_host_bus_data_error_enable: 28;
    pub interface_fatal_error_enable, set_interface_fatal_error_enable: 27;
    pub interface_non_fatal_error_enable, set_interface_non_fatal_error_enable: 26;
    pub fifo_overflow_enable, set_fifo_overflow_enable: 22;
    pub physical_layer_ready_change_enable, set_physical_layer_ready_change_enable: 20;
    pub device_mechanical_presence_enable, set_device_mechanical_presence_enable: 7;
    pub port_connect_status_change_enable, set_port_connect_status_change_enable: 6;
    pub descriptor_processed_enable, set_descriptor_processed_enable: 5;
    pub unknown_fis_interrupt_enable, set_unknown_fis_interrupt_enable: 4;
    pub dma_setup_fis_interrupt_enable, set_dma_setup_fis_interrupt_enable: 3;
    pub pio_setup_fis_interrupt_enable, set_pio_setup_fis_interrupt_enable: 2;
    pub device_to_host_register_fis_interrupt_enable, set_device_to_host_register_fis_interrupt_enable: 1;
}

bitfield! {
    pub struct PortInterruptStatus(u32);
    impl Debug;
    pub cold_presence_detect_enable, _: 31;
    pub task_file_error_enable, _: 30;
    pub host_bus_fatal_error_enable, _: 29;
    pub host_bus_data_error_enable, _ : 28;
    pub interface_fatal_error_enable, _ : 27;
    pub interface_non_fatal_error_enable, _ : 26;
    pub fifo_overflow_enable, _ : 22;
    pub physical_layer_ready_change_enable, _ : 20;
    pub device_mechanical_presence_enable, _ : 7;
    pub port_connect_status_change_enable, _ : 6;
    // generates an interrupt when it has finished
    pub descriptor_processed_enable, _ : 5;
    pub unknown_fis_interrupt_enable, _ : 4;
    pub dma_setup_fis_interrupt_enable, _ : 3;
    pub pio_setup_fis_interrupt_enable, _ : 2;
    pub device_to_host_register_fis_interrupt_enable, _ : 1;
}

bitfield! {
    pub struct SataError(u32);
    impl Debug;
    // Diagnostic fields
    pub exchanged, _: 26;
    pub unknown_fis_type, _: 25;
    pub transport_state_transition_error, _: 24;
    pub link_sequence_error, _: 23;
    pub handshake_error, _: 22;
    pub cyclic_redundancy_check_error, _: 21;
    pub protocol_error, _: 20;
    pub internal_error, _: 19;
    pub bit_decode_error, _: 18;
    pub communication_wake, _: 17;
    pub physical_layer_internal_error, _: 16;

    // Error fields
    pub recovered_communications_error, _: 1;
    pub recovered_data_integrity_error, _: 0;
}

bitfield! {
    pub struct PortTaskFileData(u32);
    impl Debug;
    // Error register (bits 15:8)
    pub error_code, _: 15, 8;

    // Status register (bits 7:0)
    pub busy, _: 7;
    pub data_transfer_requested, _: 3;
    pub error_occurred, _: 0;
    pub status_byte, _: 7, 0;
}

bitfield! {
    pub struct AtaError(u8);
    impl Debug;
    pub interface_cyclic_redundancy_check_error, _: 7;
    pub uncorrectable_data_error, _: 6;
    pub media_changed, _: 5;
    pub identifier_not_found, _: 4;
    pub media_change_requested, _: 3;
    pub command_aborted, _: 2;
    pub track_zero_not_found, _: 1;
    pub address_mark_not_found, _: 0;
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

    // Create a SATA instance that is only able to read the ports
    pub fn ports(base: VirtAddr) -> AhciSataPorts {
        AhciSataPorts { base }
    }

    pub fn new(
        base: VirtAddr,
        max_cmd_slots: u64,
        hba_idx: usize,
        ports_idx: usize,
    ) -> Option<Self> {
        let ports = Self::ports(base);

        let sig = ports.read_signature();
        const ATAPI_SIG: u32 = 0xEB140101;
        const SATA_SIG: u32 = 0x00000101;
        if sig == ATAPI_SIG {
            return None;
        }

        if sig != SATA_SIG {
            return None;
        }

        let status = PortStatus(ports.read_sata_status());

        // if there is no device, or there is no phys this device is unusable
        if status.device_detection() == PortStatus::DET_NOT_PRESENT
            || status.device_detection() == PortStatus::DET_PRESENT_NO_PHY
        {
            return None;
        }

        log!("{:b}", status.0);

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

        Some(Self {
            ports: AhciSataPorts { base },
            dma_20kb_buffer_vaddr: get_hhdm_offset() + frames[0].start_address().as_u64(),
            dma_20kb_buffer_paddr: frames[0].start_address(),
            max_cmd_slots,
            sector_count: 0,
            sectors_per_track: 0,
            hba_idx,
            ports_idx,
        })
    }

    pub fn is_idle(&mut self) -> bool {
        let cmd_status = self.ports.read_command_and_status();

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

    fn reset_cmd(&mut self) {
        let mut cmd_status = self.ports.read_command_and_status();

        cmd_status &= !(Self::START | Self::FIS_RECEIVE_ENABLE);

        self.ports.write_command_and_status(cmd_status);
    }

    pub fn reset(&mut self) -> Result<(), TimeOut> {
        if self.is_idle() {
            return Ok(());
        }

        self.reset_cmd();

        let time = Instant::now();
        loop {
            let cmd_status = self.ports.read_command_and_status();
            let cur = Instant::now();
            if cur - time > Duration::from_secs(1) {
                return Err(TimeOut {});
            }

            if cmd_status & (Self::COMMAND_LIST_RUNNING | Self::FIS_RECEIVE_RUNNING) == 0 {
                break;
            }
        }

        Ok(())
    }

    pub async fn failure_reset(&mut self) {
        self.reset_cmd();

        todo!()
    }

    pub fn init(&mut self) -> Result<(), TimeOut> {
        self.disable_interrupts();

        let status = PortStatus(self.ports.read_sata_status());
        // if it's offline wake it up first
        if status.device_detection() == PortStatus::DET_OFFLINE
            || status.interface_power_management() == PortStatus::IPM_NOT_PRESENT
        {
            self.reset_cmd();
            let mut control_port = PortControl(self.ports.read_sata_control());
            control_port.set_det_init(PortControl::DET_COMRESET);
            self.ports.write_sata_control(control_port.0);

            let start = Instant::now();

            loop {
                if PortStatus(self.ports.read_sata_status()).device_detection()
                    == PortStatus::DET_PRESENT_WITH_PHY
                {
                    break;
                }

                let now = Instant::now();
                if now - start >= Duration::from_secs(1) {
                    return Err(TimeOut {});
                }
            }

            self.reset_cmd();
            let mut control_port = PortControl(self.ports.read_sata_control());
            control_port.set_det_init(PortControl::DET_NO_ACTION);
            self.ports.write_sata_control(control_port.0);
        }

        // if it's in sleep wake it up first
        if status.interface_power_management() != PortStatus::IPM_ACTIVE {
            let mut cmd_status = PortCmdAndStatus(self.ports.read_command_and_status());
            const ACTIVE: u32 = 1;
            cmd_status.set_interface_comm_control(ACTIVE);
            self.ports.write_command_and_status(cmd_status.0);

            let start = Instant::now();

            loop {
                if PortStatus(self.ports.read_sata_status()).interface_power_management()
                    == PortStatus::IPM_ACTIVE
                {
                    break;
                }

                let now = Instant::now();
                if now - start >= Duration::from_secs(1) {
                    return Err(TimeOut {});
                }
            }

            self.reset_cmd();
            let mut control_port = PortControl(self.ports.read_sata_control());
            control_port.set_det_init(PortControl::DET_NO_ACTION);
            self.ports.write_sata_control(control_port.0);
        }

        self.reset()?;

        self.ports
            .write_command_list_base_lower(self.dma_20kb_buffer_paddr.as_u64() as u32);
        self.ports
            .write_command_list_base_higher((self.dma_20kb_buffer_paddr.as_u64() >> 32) as u32);

        let received_fis_area = self.dma_20kb_buffer_paddr.as_u64() + RECEIVED_FIS_AREA_OFFSET;

        self.ports.write_fis_base_lower(received_fis_area as u32);
        self.ports
            .write_fis_base_higher((received_fis_area >> 32) as u32);

        // resets sata error
        self.ports.write_sata_error(0xFFFFFFFF);
        // this only writes to the non-reserved bits
        // self.write_sata_error(0b00000_11111_11111_1_0000_1111_000000_11);
        self.ports.write_interrupt_status(0);

        let start = Instant::now();
        loop {
            let tfd = self.ports.read_task_file_data();
            if (tfd & 0x88) == 0 {
                break;
            } // BSY and DRQ are bits 7 and 3
            if Instant::now() - start > Duration::from_secs(1) {
                log!("Timeout waiting for port to become non-busy");
                return Err(TimeOut {});
            }
        }

        let mut cmd = PortCmdAndStatus(self.ports.read_command_and_status());
        cmd.set_fis_recv_enable(true);
        self.ports.write_command_and_status(cmd.0);

        while !PortCmdAndStatus(self.ports.read_command_and_status()).fis_recv_running() {
            core::hint::spin_loop();
        }

        cmd.set_start(true);
        self.ports.write_command_and_status(cmd.0);

        while !PortCmdAndStatus(self.ports.read_command_and_status()).cmd_list_running() {
            core::hint::spin_loop();
        }

        log!("Reset complete");

        self.identify();
        self.enable_interrupts();

        Ok(())
    }

    fn enable_interrupts(&mut self) {
        let mut interrupts = PortInterruptEnable(0);
        interrupts.set_task_file_error_enable(true);
        interrupts.set_descriptor_processed_enable(true);
        self.ports.write_interrupt_enable(interrupts.0);
    }

    fn disable_interrupts(&mut self) {
        self.ports.write_interrupt_enable(0);
    }

    fn nth_command_table_offset(n: u64) -> u64 {
        CMD_TABLES_OFFSET + n * CMD_TABLE_SIZE
    }

    pub fn identify(&mut self) {
        let cmd_tables_phys_addr = (self.dma_20kb_buffer_paddr + CMD_TABLES_OFFSET).as_u64();
        // use the first slot
        let buf = self.get_buffer();

        // this is to make sure the buffer is 32 bytes aligned
        let result_buf = vec![0u32; SECTOR_SIZE / 4].into_boxed_slice();
        let result_buf_ptr = (result_buf.as_ptr() as u64) - get_hhdm_offset().as_u64();

        let cmd_table: &mut CommandTable = bytemuck::from_bytes_mut(
            &mut buf[Self::nth_command_table_offset(0) as usize
                ..Self::nth_command_table_offset(0) as usize + size_of::<CommandTable>()],
        );

        let mut fis_flags = FisRegH2DFlags(0);
        fis_flags.set_is_command(true);
        fis_flags.set_port_multiplier(0);

        cmd_table.cmd_fis = fis::FisRegH2D {
            command: AtaCommand::Identify as u8,
            flags: fis_flags.0,
            ..Default::default()
        };

        let mut prdt_flags = PrdtEntryFlags(0);
        prdt_flags.set_interrupt(false);
        prdt_flags.set_byte_count(SECTOR_SIZE as u32 - 1);

        cmd_table.prdt_table[0] = PrdtEntry {
            data_base_low: result_buf_ptr as u32,
            data_base_high: (result_buf_ptr >> 32) as u32,
            flags: prdt_flags.0,
            ..Default::default()
        };

        let cmd_header: &mut CommandHeader =
            bytemuck::from_bytes_mut(&mut buf[0..size_of::<CommandHeader>()]);

        let mut cmd_header_flags = CommandHeaderFlags(0);
        cmd_header_flags.set_port_multiplier(0);
        cmd_header_flags.set_clear_busy_when_r_ok(false);
        cmd_header_flags.set_bist(0);
        cmd_header_flags.set_reset(0);
        cmd_header_flags.set_is_prefetchable(false);
        cmd_header_flags.set_is_atapi(false);
        cmd_header_flags.set_is_write(false);
        cmd_header_flags.set_cmd_fis_len((size_of::<fis::FisRegH2D>() / size_of::<u32>()) as u16);

        cmd_header.physical_region_descriptor_table_length = 1;
        cmd_header.flags = cmd_header_flags.0;
        cmd_header.physical_region_descriptor_bytes_count = 0;

        cmd_header.cmd_table_base_addr_low = cmd_tables_phys_addr as u32;
        cmd_header.cmd_table_base_addr_high = (cmd_tables_phys_addr >> 32) as u32;

        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);

        let mut cmd_issue = self.ports.read_command_issue();
        cmd_issue &= !0x1;
        cmd_issue |= 0x1;
        self.ports.write_command_issue(cmd_issue);

        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);

        loop {
            if self.ports.read_command_issue() & 0x1 == 0 {
                break;
            }

            core::hint::spin_loop();
        }

        let tfd = self.ports.read_task_file_data();
        if (tfd & 0x01) != 0 {
            // Bit 0 is the Error bit
            panic!("The disk reported an error (TFD: {:#x})", tfd);
        }

        if (tfd & 0x80) != 0 || (tfd & 0x08) != 0 {
            panic!("The disk is still busy or requesting data despite CI being 0!");
        }

        let identify_data = &unsafe { *(result_buf.as_ptr() as *const IdentifyData) };

        log!("{:?}", identify_data);

        self.sectors_per_track = identify_data.sectors_per_track;
        self.sector_count = identify_data.lba48_sectors;
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

    fn sector_count(&mut self) -> u64 {
        self.sector_count
    }

    fn sectors_per_track(&mut self) -> u16 {
        self.sectors_per_track
    }

    fn run<'device, 'rx, 'future>(
        &'device mut self,
        rx: &'rx UnboundedReceiver<HalStorageOperation>,
    ) -> core::pin::Pin<Box<dyn Future<Output = ()> + 'future + Send + Sync>>
    where
        'rx: 'future,
        'device: 'future,
    {
        Box::pin(async move { self.run_task(rx).await })
    }
}

#[derive(Debug)]
pub struct AhciSataPorts {
    base: VirtAddr,
}

impl AhciSataPorts {
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
