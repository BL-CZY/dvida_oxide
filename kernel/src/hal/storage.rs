use crate::drivers::ata::pata::{pio::PataPioIoErr, PATA_DEVICES};
use alloc::vec;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::Mutex;

enum DeviceType {
    Unidentified,
    /// I only know this lol
    PataPio,
    /// I might be able to do this in the future
    PataDma,
    /// Trust me I will do it
    Sata,
    /// no
    Nvme,
}

pub const PRIMARY: usize = 0;
pub const SECONDARY: usize = 1;

#[derive(Debug)]
pub enum IoErr {
    Unavailable,
    PataPio(PataPioIoErr),
}

pub struct HalStorageDevice {
    device_io_type: DeviceType,
    device_loc: usize,
    available: bool,
}

lazy_static! {
    pub static ref STORAGE_CONTEXT_ARR: Vec<Mutex<HalStorageDevice>> = vec![
        Mutex::new(HalStorageDevice::new(PRIMARY)),
        Mutex::new(HalStorageDevice::new(SECONDARY))
    ];
}

impl HalStorageDevice {
    pub fn new(loc: usize) -> Self {
        HalStorageDevice {
            device_io_type: DeviceType::Unidentified,
            device_loc: loc,
            available: false,
        }
    }

    pub fn init(&mut self) {
        unsafe {
            if let Ok(()) = PATA_DEVICES[self.device_loc as usize].lock().identify() {
                self.available = true;
                self.device_io_type = DeviceType::PataPio;
            }
        }
    }

    pub fn highest_lba(&self) -> u64 {
        match self.device_io_type {
            DeviceType::PataPio | DeviceType::PataDma => {
                PATA_DEVICES[self.device_loc as usize].lock().highest_lba()
            }
            _ => 0,
        }
    }

    pub fn sectors_per_track(&self) -> u16 {
        match self.device_io_type {
            DeviceType::PataPio | DeviceType::PataDma => {
                PATA_DEVICES[self.device_loc as usize]
                    .lock()
                    .sectors_per_track
            }
            _ => 0,
        }
    }

    pub fn read_sectors(&mut self, index: i64, count: u16) -> Result<Vec<u8>, IoErr> {
        if !self.available {
            return Err(IoErr::Unavailable);
        }

        match self.device_io_type {
            DeviceType::PataPio => match PATA_DEVICES[self.device_loc as usize]
                .lock()
                .pio_read_sectors(index, count)
            {
                Ok(res) => Ok(res),
                Err(e) => Err(IoErr::PataPio(e)),
            },
            _ => Ok(vec![]),
        }
    }

    pub fn write_sectors(&mut self, index: i64, count: u16, input: &Vec<u8>) -> Result<(), IoErr> {
        if !self.available {
            return Err(IoErr::Unavailable);
        }

        match self.device_io_type {
            DeviceType::PataPio => match PATA_DEVICES[self.device_loc as usize]
                .lock()
                .pio_write_sectors(index, count, input)
            {
                Ok(()) => Ok(()),
                Err(e) => Err(IoErr::PataPio(e)),
            },

            _ => Ok(()),
        }
    }
}
