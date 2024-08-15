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

#[derive(PartialEq, Clone, Copy)]
#[repr(C)]
enum DeviceLoc {
    Primary = 0,
    Secondary,
}

pub enum IoErr {
    Unavailable,
    PataPio(PataPioIoErr),
}

pub struct HalStorageDevice {
    device_io_type: DeviceType,
    device_loc: DeviceLoc,
    available: bool,
}

lazy_static! {
    pub static ref PRIMARY_STORAGE_CONTEXT: Mutex<HalStorageDevice> =
        Mutex::new(HalStorageDevice::new(DeviceLoc::Primary));
    pub static ref SECONDARY_STORAGE_CONTEXT: Mutex<HalStorageDevice> =
        Mutex::new(HalStorageDevice::new(DeviceLoc::Secondary));
}

impl HalStorageDevice {
    pub fn new(loc: DeviceLoc) -> Self {
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

    pub fn write_sectors(
        &mut self,
        index: i64,
        count: u16,
        input: &mut Vec<u8>,
    ) -> Result<(), IoErr> {
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
