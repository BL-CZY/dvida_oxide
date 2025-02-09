use crate::drivers::ata::pata::PATA_DEVICES;
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::Mutex;
use thiserror::Error;

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

#[derive(Debug, Error)]
pub enum IoErr {
    #[error("The device is unavailable")]
    Unavailable,
    #[error("The device is unimplemented")]
    Unimplemented,
    #[error("Sector is out of range")]
    SectorOutOfRange,
    #[error("The initialization timed out")]
    InitTimeout,
    #[error("The IO process timed out")]
    IOTimeout,
    #[error("The cache flush process timed out")]
    FlushCacheTimeout,
    #[error("Input buffer is too small")]
    InputTooSmall,
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

    pub fn read_sectors(
        &mut self,
        index: i64,
        count: u16,
    ) -> Result<Vec<u8>, Box<dyn core::error::Error>> {
        if !self.available {
            return Err(Box::new(IoErr::Unavailable));
        }

        match self.device_io_type {
            DeviceType::PataPio => Ok(PATA_DEVICES[self.device_loc as usize]
                .lock()
                .pio_read_sectors(index, count)?),
            _ => Err(Box::new(IoErr::Unimplemented)),
        }
    }

    pub fn write_sectors(
        &mut self,
        index: i64,
        count: u16,
        input: &Vec<u8>,
    ) -> Result<(), Box<dyn core::error::Error>> {
        if !self.available {
            return Err(Box::new(IoErr::Unavailable));
        }

        match self.device_io_type {
            DeviceType::PataPio => Ok(PATA_DEVICES[self.device_loc as usize]
                .lock()
                .pio_write_sectors(index, count, input)?),
            _ => Err(Box::new(IoErr::Unimplemented)),
        }
    }
}
