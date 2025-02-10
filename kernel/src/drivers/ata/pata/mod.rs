use super::{
    cmd::{IDENTITY, START_IDENTIFY},
    offsets::{COMMAND, DRIVE, ERROR, FEATURE, LBA_HIGH, LBA_LOW, LBA_MID, SECTOR_COUNT, STATUS},
};
use crate::println;
use crate::utils::binary_test;
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

pub struct PataDevice {
    identified: bool,
    lba48_supported: bool,
    lba28_sector_count: u32,
    lba48_sector_count: u64,
    pub sectors_per_track: u16,

    port: u16,
    data_port: PortGeneric<u16, ReadWriteAccess>,
    error_port_lba28: PortGeneric<u8, ReadOnlyAccess>,
    error_port_lba48: PortGeneric<u16, ReadOnlyAccess>,
    features_port_lba28: PortGeneric<u8, WriteOnlyAccess>,
    features_port_lba48: PortGeneric<u16, WriteOnlyAccess>,
    sector_count_port: PortGeneric<u8, ReadWriteAccess>,
    lba_low_port: PortGeneric<u8, ReadWriteAccess>,
    lba_mid_port: PortGeneric<u8, ReadWriteAccess>,
    lba_high_port: PortGeneric<u8, ReadWriteAccess>,
    drive_port: PortGeneric<u8, ReadWriteAccess>,
    status_port: PortGeneric<u8, ReadOnlyAccess>,
    cmd_port: PortGeneric<u8, WriteOnlyAccess>,
}

impl PataDevice {
    pub fn new(base_port: u16) -> Self {
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
        self.identified = true;

        if binary_test(buf[83].into(), 10) {
            self.lba48_supported = true;
        }

        self.sectors_per_track = buf[6];
        self.lba28_sector_count = ((buf[61] as u32) << 16) | buf[60] as u32;
        self.lba48_sector_count = ((buf[103] as u64) << 48)
            | ((buf[102] as u64) << 32)
            | ((buf[101] as u64) << 16)
            | (buf[100] as u64);

        println!("[ATA drive at port {} identify result]:", { self.port });
        println!("Is lba48 supported: {}", self.lba48_supported);
        println!("Lba28 sector count: {:x}", self.lba28_sector_count);
        println!("Lba48 sector count: {:x}", self.lba48_sector_count);
    }

    pub fn highest_lba(&self) -> u64 {
        if self.lba48_supported {
            self.lba48_sector_count
        } else {
            self.lba28_sector_count as u64
        }
    }

    pub fn identify(&mut self) -> Result<(), PataIdentErr> {
        unsafe {
            self.drive_port.write(START_IDENTIFY);

            self.sector_count_port.write(0);
            self.lba_low_port.write(0);
            self.lba_mid_port.write(0);
            self.lba_high_port.write(0);

            self.cmd_port.write(IDENTITY);

            if self.status_port.read() == 0 {
                println!("Drive doesn't exist");
                return Err(PataIdentErr::DeviceNonExist);
            }

            for _ in 0..14 {
                self.status_port.read();
            }
        }

        loop {
            unsafe {
                if self.lba_mid_port.read() != 0 || self.lba_high_port.read() != 0 {
                    println!("Device not ATA");
                    return Err(PataIdentErr::DeviceNotAta);
                }

                if (self.status_port.read() & 0b00000001) == 0b00000001 {
                    println!("Error");
                    return Err(PataIdentErr::Error);
                }

                if (self.status_port.read() & 0b00001000) == 0b00001000 {
                    break;
                }
            }
        }

        let mut buf: [u16; 256] = [0; 256];

        for i in 0..256 {
            unsafe {
                buf[i] = self.data_port.read();
            }
        }

        self.read_identify_buffer(&buf);

        Ok(())
    }
}
