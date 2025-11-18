use crate::drivers::ata::pata::{PATA_PRIMARY_BASE, PATA_SECONDARY_BASE, PataDevice};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use alloc::{boxed::Box, string::String};
use ejcineque::sync::mpsc::unbounded::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use lazy_static::lazy_static;
use once_cell_no_std::OnceCell;
use spin::Mutex;
use terminal::{iprintln, log};
use thiserror::Error;

pub static PRIMARY_STORAGE_SENDER: OnceCell<UnboundedSender<HalStorageOperation>> = OnceCell::new();
pub static SECONDARY_STORAGE_SENDER: OnceCell<UnboundedSender<HalStorageOperation>> =
    OnceCell::new();

#[derive(Debug)]
pub enum DeviceType {
    Unidentified,
    /// I only know this lol
    PataPio(PataDevice),
    /// I might be able to do this in the future
    PataDma,
    /// Trust me I will do it
    Sata,
    /// no
    Nvme,
}

pub const PRIMARY: usize = 0;
pub const SECONDARY: usize = 1;

pub const BLOCK_SIZE: usize = 512;

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

#[derive(Debug)]
pub struct HalStorageDevice {
    pub device_io_type: DeviceType,
    pub device_loc: usize,
    pub available: bool,
    pub pata_port: u16,
}

#[derive(Debug)]
pub enum HalStorageOperation {
    Read {
        buffer: Box<[u8]>,
        lba: i64,
        sender: UnboundedSender<HalStorageOperationResult>,
    },

    Write {
        buffer: Box<[u8]>,
        lba: i64,
        sender: UnboundedSender<HalStorageOperationResult>,
    },
}

#[derive(Debug)]
pub enum HalStorageOperationResult {
    Success,
    Failure(String),
}

lazy_static! {
    pub static ref STORAGE_CONTEXT_ARR: Vec<Mutex<HalStorageDevice>> = vec![
        Mutex::new(HalStorageDevice::new(PRIMARY, PATA_PRIMARY_BASE)),
        Mutex::new(HalStorageDevice::new(SECONDARY, PATA_SECONDARY_BASE))
    ];
}

/// After this is executed nothing should directly access those structs
pub async fn run_storage_device(index: usize) {
    let rx = if index == PRIMARY {
        let (primary_storage_tx, rx) = unbounded_channel::<HalStorageOperation>();
        let _ = PRIMARY_STORAGE_SENDER
            .set(primary_storage_tx)
            .expect("Failed to put the primary storage sender");

        rx
    } else {
        let (secondary_storage_tx, rx) = unbounded_channel::<HalStorageOperation>();
        let _ = SECONDARY_STORAGE_SENDER
            .set(secondary_storage_tx)
            .expect("Failed to put the secondary storage sender");

        rx
    };

    log!("Sender initialization complete!");
    STORAGE_CONTEXT_ARR[index].lock().run(rx).await;
}

impl HalStorageDevice {
    pub fn new(loc: usize, pata_port: u16) -> Self {
        HalStorageDevice {
            device_io_type: DeviceType::Unidentified,
            device_loc: loc,
            available: false,
            pata_port,
        }
    }

    pub async fn run(&mut self, rx: UnboundedReceiver<HalStorageOperation>) {
        if !self.available {
            iprintln!(
                "Failed to run the storage device at {:x} because it is not available",
                self.pata_port
            );
            return;
        }

        while let Some(op) = rx.recv().await {
            iprintln!("Received storage operation: {:?}", op);
            match op {
                HalStorageOperation::Read {
                    mut buffer,
                    lba,
                    sender,
                } => {
                    match self
                        .read_sectors_async(
                            lba,
                            (buffer.len() / BLOCK_SIZE) as u16,
                            buffer.as_mut(),
                        )
                        .await
                    {
                        Ok(_) => {
                            iprintln!("Read Operation succeeded!: {:?}", buffer);
                            sender.send(HalStorageOperationResult::Success);
                        }
                        Err(e) => {
                            iprintln!("Operation failed..: {:?}", e);
                            sender.send(HalStorageOperationResult::Failure(e.to_string()));
                        }
                    }
                }

                HalStorageOperation::Write {
                    buffer,
                    lba,
                    sender,
                } => {
                    match self
                        .write_sectors_async(lba, (buffer.len() / BLOCK_SIZE) as u16, &buffer)
                        .await
                    {
                        Ok(_) => {
                            iprintln!("Write operation succeeded!: {:?}", buffer);
                            sender.send(HalStorageOperationResult::Success);
                        }

                        Err(e) => {
                            iprintln!("Write operation failed..: {:?}", e);
                            sender.send(HalStorageOperationResult::Failure(e.to_string()));
                        }
                    }
                }
            }
        }
    }

    pub fn init(&mut self) {
        // test pata
        let mut pata = PataDevice::new(self.pata_port);
        if let Ok(()) = pata.identify() {
            self.available = true;
            self.device_io_type = DeviceType::PataPio(pata);
            return;
        }
    }

    pub fn sector_count(&self) -> u64 {
        match self.device_io_type {
            DeviceType::PataPio(ref pata) => pata.sector_count(),
            _ => 0,
        }
    }

    pub fn sectors_per_track(&self) -> u16 {
        match self.device_io_type {
            DeviceType::PataPio(ref pata) => pata.sectors_per_track,
            _ => 0,
        }
    }

    pub fn read_sectors(
        &mut self,
        index: i64,
        count: u16,
        output: &mut [u8],
    ) -> Result<(), Box<dyn core::error::Error>> {
        if !self.available {
            return Err(Box::new(IoErr::Unavailable));
        }

        match self.device_io_type {
            DeviceType::PataPio(ref mut pata) => Ok(pata.pio_read_sectors(index, count, output)?),
            _ => Err(Box::new(IoErr::Unimplemented)),
        }
    }

    pub async fn read_sectors_async(
        &mut self,
        index: i64,
        count: u16,
        output: &mut [u8],
    ) -> Result<(), Box<dyn core::error::Error>> {
        if !self.available {
            return Err(Box::new(IoErr::Unavailable));
        }

        match self.device_io_type {
            DeviceType::PataPio(ref mut pata) => {
                Ok(pata.pio_read_sectors_async(index, count, output).await?)
            }
            _ => Err(Box::new(IoErr::Unimplemented)),
        }
    }

    pub fn write_sectors(
        &mut self,
        index: i64,
        count: u16,
        input: &[u8],
    ) -> Result<(), Box<dyn core::error::Error>> {
        if !self.available {
            return Err(Box::new(IoErr::Unavailable));
        }

        match self.device_io_type {
            DeviceType::PataPio(ref mut pata) => Ok(pata.pio_write_sectors(index, count, input)?),
            _ => Err(Box::new(IoErr::Unimplemented)),
        }
    }

    pub async fn write_sectors_async(
        &mut self,
        index: i64,
        count: u16,
        input: &[u8],
    ) -> Result<(), Box<dyn core::error::Error>> {
        if !self.available {
            return Err(Box::new(IoErr::Unavailable));
        }

        match self.device_io_type {
            DeviceType::PataPio(ref mut pata) => {
                Ok(pata.pio_write_sectors_async(index, count, input).await?)
            }
            _ => Err(Box::new(IoErr::Unimplemented)),
        }
    }
}

pub async fn read_sectors(
    index: usize,
    buffer: Box<[u8]>,
    lba: i64,
) -> Result<(), HalStorageOperationErr> {
    let sender = if index == PRIMARY {
        PRIMARY_STORAGE_SENDER.get().unwrap().clone()
    } else {
        SECONDARY_STORAGE_SENDER.get().unwrap().clone()
    };

    let (tx, rx) = unbounded_channel::<HalStorageOperationResult>();

    sender.send(HalStorageOperation::Read {
        buffer,
        lba,
        sender: tx,
    });

    if let Some(res) = rx.recv().await {
        if let HalStorageOperationResult::Failure(err) = res {
            Err(HalStorageOperationErr::DriveErr(err))
        } else {
            Ok(())
        }
    } else {
        Err(HalStorageOperationErr::DriveDidntRespond)
    }
}

pub async fn write_sectors(
    index: usize,
    buffer: Box<[u8]>,
    lba: i64,
) -> Result<(), HalStorageOperationErr> {
    let sender = if index == PRIMARY {
        PRIMARY_STORAGE_SENDER.get().unwrap().clone()
    } else {
        SECONDARY_STORAGE_SENDER.get().unwrap().clone()
    };

    let (tx, rx) = unbounded_channel::<HalStorageOperationResult>();

    sender.send(HalStorageOperation::Write {
        buffer,
        lba,
        sender: tx,
    });

    if let Some(res) = rx.recv().await {
        if let HalStorageOperationResult::Failure(str) = res {
            Err(HalStorageOperationErr::DriveErr(str))
        } else {
            Ok(())
        }
    } else {
        Err(HalStorageOperationErr::DriveDidntRespond)
    }
}

#[derive(Debug, Clone, Error)]
pub enum HalStorageOperationErr {
    #[error("Drive didn't respond")]
    DriveDidntRespond,
    #[error("Drive responded with error: {0}")]
    DriveErr(String),
}
