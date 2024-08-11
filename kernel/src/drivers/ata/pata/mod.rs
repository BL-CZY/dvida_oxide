use super::{
    cmd::{IDENTITY, START_IDENTIFY},
    offsets::{COMMAND, DRIVE, ERROR, FEATURE, LBA_HIGH, LBA_LOW, LBA_MID, SECTOR_COUNT, STATUS},
};
use crate::println;
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::instructions::port::{
    Port, PortGeneric, PortReadOnly, PortWriteOnly, ReadOnlyAccess, ReadWriteAccess,
    WriteOnlyAccess,
};

pub mod pio;

lazy_static! {
    pub static ref PRIMARY_PATA: Mutex<PATADevice> = Mutex::new(PATADevice::new(0x1F0));
}

pub struct PATADevice {
    port: u16,
    data_port: PortGeneric<u16, ReadWriteAccess>,
    error_port_lba28: PortGeneric<u8, ReadOnlyAccess>,
    error_port_lba48: PortGeneric<u16, ReadOnlyAccess>,
    features_port_lba28: PortGeneric<u8, WriteOnlyAccess>,
    features_port_lba48: PortGeneric<u16, WriteOnlyAccess>,
    sector_count_port_lba28: PortGeneric<u8, ReadWriteAccess>,
    sector_count_port_lba48: PortGeneric<u16, ReadWriteAccess>,
    lba_low_port_lba28: PortGeneric<u8, ReadWriteAccess>,
    lba_low_port_lba48: PortGeneric<u16, ReadWriteAccess>,
    lba_mid_port_lba28: PortGeneric<u8, ReadWriteAccess>,
    lba_mid_port_lba48: PortGeneric<u16, ReadWriteAccess>,
    lba_high_port_lba28: PortGeneric<u8, ReadWriteAccess>,
    lba_high_port_lba48: PortGeneric<u16, ReadWriteAccess>,
    drive_port: PortGeneric<u8, ReadWriteAccess>,
    status_port: PortGeneric<u8, ReadOnlyAccess>,
    cmd_port: PortGeneric<u8, WriteOnlyAccess>,
}

impl PATADevice {
    pub fn new(base_port: u16) -> Self {
        PATADevice {
            port: base_port,
            data_port: Port::new(base_port),
            error_port_lba28: PortReadOnly::new(base_port + ERROR),
            error_port_lba48: PortReadOnly::new(base_port + ERROR),
            features_port_lba28: PortWriteOnly::new(base_port + FEATURE),
            features_port_lba48: PortWriteOnly::new(base_port + FEATURE),
            sector_count_port_lba28: Port::new(base_port + SECTOR_COUNT),
            sector_count_port_lba48: Port::new(base_port + SECTOR_COUNT),
            lba_low_port_lba28: Port::new(base_port + LBA_LOW),
            lba_low_port_lba48: Port::new(base_port + LBA_LOW),
            lba_mid_port_lba28: Port::new(base_port + LBA_MID),
            lba_mid_port_lba48: Port::new(base_port + LBA_MID),
            lba_high_port_lba28: Port::new(base_port + LBA_HIGH),
            lba_high_port_lba48: Port::new(base_port + LBA_MID),
            drive_port: Port::new(base_port + DRIVE),
            status_port: PortReadOnly::new(base_port + STATUS),
            cmd_port: PortWriteOnly::new(base_port + COMMAND),
        }
    }

    pub unsafe fn identify(&mut self) {
        self.drive_port.write(START_IDENTIFY);

        self.sector_count_port_lba28.write(0);
        self.lba_low_port_lba28.write(0);
        self.lba_mid_port_lba28.write(0);
        self.lba_high_port_lba28.write(0);

        self.cmd_port.write(IDENTITY);

        if self.status_port.read() == 0 {
            println!("Drive doesn't exist");
            return;
        }

        for _ in 0..14 {
            self.status_port.read();
        }

        loop {
            if self.lba_mid_port_lba28.read() != 0 || self.lba_high_port_lba28.read() != 0 {
                println!("Device not ATA");
                return;
            }

            if (self.status_port.read() & 0b00000001) == 0b00000001 {
                println!("Error");
                return;
            }

            if (self.status_port.read() & 0b00001000) == 0b00001000 {
                break;
            }
        }

        let mut buf: [u16; 256] = [0; 256];

        for i in 0..256 {
            buf[i] = self.data_port.read();
        }

        println!("{:?}", buf);
    }
}
