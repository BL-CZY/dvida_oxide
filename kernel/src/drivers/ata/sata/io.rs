use crate::{
    arch::x86_64::memory::get_hhdm_offset,
    drivers::ata::sata::{
        AhciSata,
        command::{CommandHeader, CommandHeaderFlags, CommandTable, PrdtEntry, PrdtEntryFlags},
        fis::{self, AtaCommand, DEVICE_LBA_MODE, FORCE_UNIT_FLUSH, FisRegH2DFlags},
    },
    hal::{buffer::Buffer, storage::SECTOR_SIZE},
    log,
};

impl AhciSata {
    fn lba48_supported(&self) -> bool {
        const LBA_48_SUPPORTED_MASK: u16 = 0x1 << 10;
        self.identify_data.command_set_supported2 & LBA_48_SUPPORTED_MASK != 0
    }

    pub async fn start_read_sectors(&mut self, cmd_queue_idx: usize, lba: i64, buffer: Buffer) {
        // only supports lba48
        if !self.lba48_supported() {
            return;
        }

        let count = (buffer.len() / SECTOR_SIZE) as u16;

        let lba: u64 = if lba < 0 {
            // TODO: correct this after fixing the infinit loop
            self.identify_data.lba48_sectors - lba as u64
        } else {
            lba as u64
        };

        log!("start read at lba: {lba} and sector count: {count}");

        let cmd_tables_phys_addr = (self.dma_20kb_buffer_paddr
            + Self::nth_command_table_offset(cmd_queue_idx as u64))
        .as_u64();
        // use the first slot
        let buf = self.get_buffer();

        // this is to make sure the buffer is 32 bytes aligned
        let result_buf_ptr = (buffer.inner as u64) - get_hhdm_offset().as_u64();
        assert_eq!(result_buf_ptr % 4, 0);

        let cmd_table: &mut CommandTable = bytemuck::from_bytes_mut(
            &mut buf[Self::nth_command_table_offset(cmd_queue_idx as u64) as usize
                ..Self::nth_command_table_offset(cmd_queue_idx as u64) as usize
                    + size_of::<CommandTable>()],
        );

        let mut fis_flags = FisRegH2DFlags(0);
        fis_flags.set_is_command(true);
        fis_flags.set_port_multiplier(0);

        cmd_table.cmd_fis = fis::FisRegH2D {
            command: AtaCommand::ReadDmaExt as u8,
            flags: fis_flags.0,
            lba0: lba as u8,
            lba1: (lba >> 8) as u8,
            lba2: (lba >> 16) as u8,
            lba3: (lba >> 24) as u8,
            lba4: (lba >> 32) as u8,
            lba5: (lba >> 40) as u8,
            count_low: count as u8,
            count_high: (count >> 8) as u8,
            device: DEVICE_LBA_MODE,
            ..Default::default()
        };

        let mut prdt_flags = PrdtEntryFlags(0);
        prdt_flags.set_interrupt(false);
        prdt_flags.set_byte_count((count as u32 * SECTOR_SIZE as u32) - 1);

        cmd_table.prdt_table[0] = PrdtEntry {
            data_base_low: result_buf_ptr as u32,
            data_base_high: (result_buf_ptr >> 32) as u32,
            flags: prdt_flags.0,
            ..Default::default()
        };

        let cmd_header: &mut CommandHeader = bytemuck::from_bytes_mut(
            &mut buf[cmd_queue_idx * size_of::<CommandHeader>()
                ..cmd_queue_idx * size_of::<CommandHeader>() + size_of::<CommandHeader>()],
        );

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

        self.ports.write_interrupt_status(0xFFFFFFFF);
        self.ports.write_sata_error(0xFFFFFFFF);
        self.hba_ports.write_interrupt_status(0xFFFFFFFF);

        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);

        self.ports.write_command_issue(0x1 << cmd_queue_idx);

        // loop {
        //     if self.ports.read_command_issue() & (0x1 << cmd_queue_idx) == 0 {
        //         break;
        //     }
        //
        //     core::hint::spin_loop();
        // }
        // log!(
        //     "After Poll - PxIS: {:#b}",
        //     self.ports.read_interrupt_status()
        // );
        //
        // self.ports
        //     .write_interrupt_status(self.ports.read_interrupt_status());
        //
        // log!(
        //     "After Poll & overwrite - PxIS: {:#b}",
        //     self.ports.read_interrupt_status()
        // );
        //
        // let tfd = self.ports.read_task_file_data();
        // if (tfd & 0x01) != 0 {
        //     // Bit 0 is the Error bit
        //     panic!("The disk reported an error (TFD: {:#x})", tfd);
        // }
        //
        // if (tfd & 0x80) != 0 || (tfd & 0x08) != 0 {
        //     panic!("The disk is still busy or requesting data despite CI being 0!");
        // }
        //
        // log!("{}", buffer);
    }

    /// this will be mainly used for page cache, the buffer will be a page
    /// doesn't check the 4gib boundary
    pub async fn start_write_sectors(&mut self, cmd_queue_idx: usize, lba: i64, buffer: Buffer) {
        // only supports lba48
        if !self.lba48_supported() {
            return;
        }

        let count = (buffer.len() / SECTOR_SIZE) as u16;

        let lba: u64 = if lba < 0 {
            self.identify_data.lba48_sectors - lba as u64
        } else {
            lba as u64
        };

        let cmd_tables_phys_addr = (self.dma_20kb_buffer_paddr
            + Self::nth_command_table_offset(cmd_queue_idx as u64))
        .as_u64();
        // use the first slot
        let buf = self.get_buffer();

        // this is to make sure the buffer is 32 bytes aligned
        let result_buf_ptr = (buffer.inner as u64) - get_hhdm_offset().as_u64();
        assert_eq!(result_buf_ptr % 4, 0);

        let cmd_table: &mut CommandTable = bytemuck::from_bytes_mut(
            &mut buf[Self::nth_command_table_offset(cmd_queue_idx as u64) as usize
                ..Self::nth_command_table_offset(cmd_queue_idx as u64) as usize
                    + size_of::<CommandTable>()],
        );

        let mut fis_flags = FisRegH2DFlags(0);
        fis_flags.set_is_command(true);
        fis_flags.set_port_multiplier(0);

        cmd_table.cmd_fis = fis::FisRegH2D {
            command: AtaCommand::WriteDmaExt as u8,
            flags: fis_flags.0,
            lba0: lba as u8,
            lba1: (lba >> 8) as u8,
            lba2: (lba >> 16) as u8,
            lba3: (lba >> 24) as u8,
            lba4: (lba >> 32) as u8,
            lba5: (lba >> 40) as u8,
            count_low: count as u8,
            count_high: (count >> 8) as u8,
            device: DEVICE_LBA_MODE | FORCE_UNIT_FLUSH,
            ..Default::default()
        };

        let mut prdt_flags = PrdtEntryFlags(0);
        prdt_flags.set_interrupt(false);
        prdt_flags.set_byte_count((count as u32 * SECTOR_SIZE as u32) - 1);

        cmd_table.prdt_table[0] = PrdtEntry {
            data_base_low: result_buf_ptr as u32,
            data_base_high: (result_buf_ptr >> 32) as u32,
            flags: prdt_flags.0,
            ..Default::default()
        };

        let cmd_header: &mut CommandHeader = bytemuck::from_bytes_mut(
            &mut buf[cmd_queue_idx * size_of::<CommandHeader>()
                ..cmd_queue_idx * size_of::<CommandHeader>() + size_of::<CommandHeader>()],
        );

        let mut cmd_header_flags = CommandHeaderFlags(0);
        cmd_header_flags.set_port_multiplier(0);
        cmd_header_flags.set_clear_busy_when_r_ok(false);
        cmd_header_flags.set_bist(0);
        cmd_header_flags.set_reset(0);
        cmd_header_flags.set_is_prefetchable(false);
        cmd_header_flags.set_is_atapi(false);
        cmd_header_flags.set_is_write(true);
        cmd_header_flags.set_cmd_fis_len((size_of::<fis::FisRegH2D>() / size_of::<u32>()) as u16);

        cmd_header.physical_region_descriptor_table_length = 1;
        cmd_header.flags = cmd_header_flags.0;
        cmd_header.physical_region_descriptor_bytes_count = 0;

        cmd_header.cmd_table_base_addr_low = cmd_tables_phys_addr as u32;
        cmd_header.cmd_table_base_addr_high = (cmd_tables_phys_addr >> 32) as u32;

        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);

        self.ports.write_command_issue(0x1 << cmd_queue_idx);
    }

    pub async fn issue_flush(&mut self, cmd_queue_idx: usize) {
        // Point to the table (same logic as your write function)
        let table_phys = self.dma_20kb_buffer_paddr.as_u64()
            + Self::nth_command_table_offset(cmd_queue_idx as u64);

        let buf = self.get_buffer();

        // Get the specific Command Table for this slot
        let table_offset = Self::nth_command_table_offset(cmd_queue_idx as u64) as usize;
        let cmd_table: &mut CommandTable = bytemuck::from_bytes_mut(
            &mut buf[table_offset..table_offset + size_of::<CommandTable>()],
        );

        let mut fis_flags = FisRegH2DFlags(0);
        fis_flags.set_is_command(true);
        fis_flags.set_port_multiplier(0);

        cmd_table.cmd_fis = fis::FisRegH2D {
            command: AtaCommand::FlushCache as u8,
            flags: fis_flags.0,
            device: DEVICE_LBA_MODE,
            ..Default::default()
        };

        // Prepare the Header
        let header_offset = cmd_queue_idx * size_of::<CommandHeader>();
        let cmd_header: &mut CommandHeader = bytemuck::from_bytes_mut(
            &mut buf[header_offset..header_offset + size_of::<CommandHeader>()],
        );

        cmd_header.physical_region_descriptor_table_length = 0; // No data transfer
        cmd_header.flags = 5; // 5 DWORDS for H2D FIS
        cmd_header.physical_region_descriptor_bytes_count = 0;

        cmd_header.cmd_table_base_addr_low = table_phys as u32;
        cmd_header.cmd_table_base_addr_high = (table_phys >> 32) as u32;

        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);

        // Kick the command issue register
        self.ports.write_command_issue(1 << cmd_queue_idx);
    }
}
