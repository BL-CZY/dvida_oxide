use crate::drivers::ata::pata::{pio::PataPioIoErr, PRIMARY_PATA, SECONDARY_PATA};
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

enum DeviceLoc {
    Primary,
    Secondary,
}

enum IoErr {
    Unavailable,
    PataPio(PataPioIoErr),
}

pub struct HalStorageContext {
    device_io_type: DeviceType,
    device_loc: DeviceLoc,
    available: bool,
}

lazy_static! {
    pub static ref PRIMARY_STORAGE_CONTEXT: Mutex<HalStorageContext> =
        Mutex::new(HalStorageContext::new(DeviceLoc::Primary));
    pub static ref SECONDARY_STORAGE_CONTEXT: Mutex<HalStorageContext> =
        Mutex::new(HalStorageContext::new(DeviceLoc::Secondary));
}

macro_rules! read_helper {
    ($self: ident, $device: ident, $index: ident, $count: ident) => {
        match $self.device_io_type {
            DeviceType::PataPio => match $device.lock().pio_read_sectors($index, $count) {
                Ok(res) => Ok(res),
                Err(e) => Err(IoErr::PataPio(e)),
            },
            _ => Ok(vec![]),
        }
    };
}

macro_rules! write_helper {
    ($self: ident, $device: ident, $index: ident, $count: ident, $input: ident) => {
        match $self.device_io_type {
            DeviceType::PataPio => match $device.lock().pio_write_sectors($index, $count, $input) {
                Ok(()) => Ok(()),
                Err(e) => Err(IoErr::PataPio(e)),
            },

            _ => Ok(()),
        }
    };
}

impl HalStorageContext {
    pub fn new(loc: DeviceLoc) -> Self {
        HalStorageContext {
            device_io_type: DeviceType::Unidentified,
            device_loc: loc,
            available: false,
        }
    }

    fn identify_primary(&mut self) {
        unsafe {
            if let Ok(()) = PRIMARY_PATA.lock().identify() {
                self.available = true;
                self.device_io_type = DeviceType::PataPio;
            }
        }
    }

    fn identify_secondary(&mut self) {
        unsafe {
            if let Ok(()) = SECONDARY_PATA.lock().identify() {
                self.available = true;
                self.device_io_type = DeviceType::PataPio;
            }
        }
    }

    pub fn init(&mut self) {
        match self.device_loc {
            DeviceLoc::Primary => {
                self.identify_primary();
            }
            DeviceLoc::Secondary => {
                self.identify_secondary();
            }
        }
    }

    pub fn read_sectors(&mut self, index: i64, count: u16) -> Result<Vec<u8>, IoErr> {
        if !self.available {
            return Err(IoErr::Unavailable);
        }

        match self.device_loc {
            DeviceLoc::Primary => read_helper!(self, PRIMARY_PATA, index, count),
            DeviceLoc::Secondary => read_helper!(self, SECONDARY_PATA, index, count),
        }
    }

    pub fn write_sectors(
        &mut self,
        index: i64,
        count: u16,
        input: &mut Vec<u8>,
    ) -> Result<(), IoErr> {
        if !self.available {
            return Err(IoErr::Unavailable);
        }

        match self.device_loc {
            DeviceLoc::Primary => write_helper!(self, PRIMARY_PATA, index, count, input),
            DeviceLoc::Secondary => write_helper!(self, SECONDARY_PATA, index, count, input),
        }
    }
}
