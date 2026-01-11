use super::{
    cmd::{IDENTITY, START_IDENTIFY},
    offsets::{COMMAND, DRIVE, ERROR, FEATURE, LBA_HIGH, LBA_LOW, LBA_MID, SECTOR_COUNT, STATUS},
};
use crate::crypto::binary_test;
use terminal::log;
use x86_64::instructions::port::{
    Port, PortGeneric, PortReadOnly, PortWriteOnly, ReadOnlyAccess, ReadWriteAccess,
    WriteOnlyAccess,
};

pub mod pio;

pub const PATA_PRIMARY_BASE: u16 = 0x1F0;
pub const PATA_SECONDARY_BASE: u16 = 0x170;

pub enum PataIdentErr {
    DeviceNonExist,
    DeviceNotAta,
    Error,
}

unsafe impl Send for PataDevice {}
unsafe impl Sync for PataDevice {}

#[derive(Debug)]
pub struct PataDevice {
    pub identified: bool,
    pub lba48_supported: bool,
    pub lba28_sector_count: u32,
    pub lba48_sector_count: u64,
    pub sectors_per_track: u16,

    pub port: u16,
    pub data_port: PortGeneric<u16, ReadWriteAccess>,
    pub error_port_lba28: PortGeneric<u8, ReadOnlyAccess>,
    pub error_port_lba48: PortGeneric<u16, ReadOnlyAccess>,
    pub features_port_lba28: PortGeneric<u8, WriteOnlyAccess>,
    pub features_port_lba48: PortGeneric<u16, WriteOnlyAccess>,
    pub sector_count_port: PortGeneric<u8, ReadWriteAccess>,
    pub lba_low_port: PortGeneric<u8, ReadWriteAccess>,
    pub lba_mid_port: PortGeneric<u8, ReadWriteAccess>,
    pub lba_high_port: PortGeneric<u8, ReadWriteAccess>,
    pub drive_port: PortGeneric<u8, ReadWriteAccess>,
    pub status_port: PortGeneric<u8, ReadOnlyAccess>,
    pub cmd_port: PortGeneric<u8, WriteOnlyAccess>,
}

impl PataDevice {
    pub fn new(base_port: u16) -> Self {
        log!("PataDevice::new: creating device at port {:#x}", base_port);
        PataDevice {
            identified: false,
            lba48_supported: false,
            lba28_sector_count: 0,
            lba48_sector_count: 0,
            sectors_per_track: 1,

            port: base_port,
            data_port: Port::new(base_port),
            error_port_lba28: PortReadOnly::new(base_port + ERROR),
            error_port_lba48: PortReadOnly::new(base_port + ERROR),
            features_port_lba28: PortWriteOnly::new(base_port + FEATURE),
            features_port_lba48: PortWriteOnly::new(base_port + FEATURE),
            sector_count_port: Port::new(base_port + SECTOR_COUNT),
            lba_low_port: Port::new(base_port + LBA_LOW),
            lba_mid_port: Port::new(base_port + LBA_MID),
            lba_high_port: Port::new(base_port + LBA_HIGH),
            drive_port: Port::new(base_port + DRIVE),
            status_port: PortReadOnly::new(base_port + STATUS),
            cmd_port: PortWriteOnly::new(base_port + COMMAND),
        }
    }

    fn read_identify_buffer(&mut self, buf: &[u16; 256]) {
        log!(
            "read_identify_buffer: parsing identify data for port {:#x}",
            self.port
        );

        self.identified = true;

        if binary_test(buf[83].into(), 10) {
            self.lba48_supported = true;
            log!("read_identify_buffer: LBA48 support detected");
        } else {
            log!("read_identify_buffer: LBA48 not supported");
        }

        self.sectors_per_track = buf[6];
        self.lba28_sector_count = ((buf[61] as u32) << 16) | buf[60] as u32;
        self.lba48_sector_count = ((buf[103] as u64) << 48)
            | ((buf[102] as u64) << 32)
            | ((buf[101] as u64) << 16)
            | (buf[100] as u64);

        log!("=== ATA Drive Identify Result (port {:#x}) ===", self.port);
        log!("  LBA48 supported: {}", self.lba48_supported);
        log!("  Sectors per track: {}", self.sectors_per_track);
        log!(
            "  LBA28 sector count: {:#x} ({} sectors)",
            self.lba28_sector_count,
            self.lba28_sector_count
        );
        log!(
            "  LBA48 sector count: {:#x} ({} sectors)",
            self.lba48_sector_count,
            self.lba48_sector_count
        );

        // Calculate and log capacity
        let capacity_bytes = if self.lba48_supported {
            self.lba48_sector_count * 512
        } else {
            self.lba28_sector_count as u64 * 512
        };
        let capacity_mb = capacity_bytes / (1024 * 1024);
        let capacity_gb = capacity_mb / 1024;
        log!("  Total capacity: {} MB ({} GB)", capacity_mb, capacity_gb);
        log!("===========================================");
    }

    pub fn sector_count(&self) -> u64 {
        if self.lba48_supported {
            self.lba48_sector_count
        } else {
            (self.lba28_sector_count) as u64
        }
    }

    pub fn identify(&mut self) -> Result<(), PataIdentErr> {
        log!(
            "identify: starting device identification at port {:#x}",
            self.port
        );

        unsafe {
            log!("identify: sending START_IDENTIFY command");
            self.drive_port.write(START_IDENTIFY);

            log!("identify: resetting sector/LBA registers");
            self.sector_count_port.write(0);
            self.lba_low_port.write(0);
            self.lba_mid_port.write(0);
            self.lba_high_port.write(0);

            log!("identify: sending IDENTITY command");
            self.cmd_port.write(IDENTITY);

            let status = self.status_port.read();
            log!("identify: initial status read: {:#x}", status);

            if status == 0 {
                log!("identify: FAILED - drive doesn't exist (status = 0)");
                return Err(PataIdentErr::DeviceNonExist);
            }

            log!("identify: performing 14 status reads for delay");
            for _ in 0..14 {
                self.status_port.read();
            }
        }

        log!("identify: waiting for device ready");
        let mut poll_count = 0;
        loop {
            poll_count += 1;

            unsafe {
                let lba_mid = self.lba_mid_port.read();
                let lba_high = self.lba_high_port.read();

                if lba_mid != 0 || lba_high != 0 {
                    log!(
                        "identify: FAILED - device not ATA (lba_mid={:#x}, lba_high={:#x})",
                        lba_mid,
                        lba_high
                    );
                    return Err(PataIdentErr::DeviceNotAta);
                }

                let status = self.status_port.read();

                if (status & 0b00000001) == 0b00000001 {
                    log!("identify: FAILED - error bit set (status={:#x})", status);
                    let error = self.error_port_lba28.read();
                    log!("identify: error register value: {:#x}", error);
                    return Err(PataIdentErr::Error);
                }

                if (status & 0b00001000) == 0b00001000 {
                    log!(
                        "identify: device ready (DRQ bit set after {} polls)",
                        poll_count
                    );
                    break;
                }

                // Log status every 1000 polls to track progress
                if poll_count % 1000 == 0 {
                    log!(
                        "identify: still waiting... (poll #{}, status={:#x})",
                        poll_count,
                        status
                    );
                }
            }
        }

        log!("identify: reading 256 words of identify data");
        let mut buf: [u16; 256] = [0; 256];

        for i in 0..256 {
            unsafe {
                buf[i] = self.data_port.read();
            }
        }

        log!("identify: data read complete, parsing results");
        self.read_identify_buffer(&buf);

        log!("identify: identification completed successfully");
        Ok(())
    }
}
