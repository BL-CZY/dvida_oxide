use crate::crypto::guid::Guid;
use crate::drivers::ata::pata::{PATA_PRIMARY_BASE, PATA_SECONDARY_BASE, PataDevice};
use crate::hal::gpt::{GPTEntry, GPTErr, GPTHeader};
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

pub const SECTOR_SIZE: usize = 512;

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
pub enum HalStorageOperation<'a> {
    Read {
        buffer: Box<[u8]>,
        lba: i64,
        sender: UnboundedSender<Result<(), HalStorageOperationErr>>,
    },

    Write {
        buffer: Box<[u8]>,
        lba: i64,
        sender: UnboundedSender<Result<(), HalStorageOperationErr>>,
    },

    InitGpt {
        force: bool,
        sender: UnboundedSender<Result<(), GPTErr>>,
    },

    ReadGpt {
        sender: UnboundedSender<Result<(GPTHeader, Vec<GPTEntry>), GPTErr>>,
    },

    AddEntry {
        name: &'a [u16; 36],
        start_lba: u64,
        end_lba: u64,
        type_guid: Guid,
        flags: u64,
        sender: UnboundedSender<Result<(), GPTErr>>,
    },

    DeleteEntry {
        idx: u32,
        sender: UnboundedSender<Result<(), GPTErr>>,
    },
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

    pub async fn run<'a>(&mut self, rx: UnboundedReceiver<HalStorageOperation<'a>>) {
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
                            (buffer.len() / SECTOR_SIZE) as u16,
                            buffer.as_mut(),
                        )
                        .await
                    {
                        Ok(_) => {
                            iprintln!("Read Operation succeeded!: {:?}", buffer);
                            sender.send(Ok(()));
                        }
                        Err(e) => {
                            iprintln!("Operation failed..: {:?}", e);
                            sender.send(Err(HalStorageOperationErr::DriveErr(e.to_string())));
                        }
                    }
                }

                HalStorageOperation::Write {
                    buffer,
                    lba,
                    sender,
                } => {
                    match self
                        .write_sectors_async(lba, (buffer.len() / SECTOR_SIZE) as u16, &buffer)
                        .await
                    {
                        Ok(_) => {
                            iprintln!("Write operation succeeded!: {:?}", buffer);
                            sender.send(Ok(()));
                        }

                        Err(e) => {
                            iprintln!("Write operation failed..: {:?}", e);
                            sender.send(Err(HalStorageOperationErr::DriveErr(e.to_string())));
                        }
                    }
                }

                HalStorageOperation::InitGpt { force, sender } => {
                    match self.create_gpt(force).await {
                        Ok(_) => {
                            log!("Initialized new gpt: forced: {}", force);
                            sender.send(Ok(()));
                        }

                        Err(e) => {
                            log!("GPT failed to initialize: {}", e);
                            sender.send(Err(e));
                        }
                    }
                }

                HalStorageOperation::ReadGpt { sender } => match self.read_gpt().await {
                    Ok(res) => {
                        log!("Read GPT table: {:?}", res);
                        sender.send(Ok(res));
                    }

                    Err(e) => {
                        log!("Failed to read GPT table: {:?}", e);
                        sender.send(Err(e));
                    }
                },

                HalStorageOperation::AddEntry {
                    name,
                    start_lba,
                    end_lba,
                    type_guid,
                    flags,
                    sender,
                } => match self
                    .add_entry(name, start_lba, end_lba, type_guid, flags)
                    .await
                {
                    Ok(res) => {
                        log!("Added GPT entry: {:?}", res);
                        sender.send(Ok(()));
                    }

                    Err(e) => {
                        log!("Failed to add GPT entry: {:?}", e);
                        sender.send(Err(e));
                    }
                },

                HalStorageOperation::DeleteEntry { idx, sender } => {
                    match self.delete_entry(idx).await {
                        Ok(res) => {
                            log!("Deleted GPT entry: {:?}", res);
                            sender.send(Ok(res));
                        }

                        Err(e) => {
                            log!("Failed to delete GPT delete: {:?}", e);
                            sender.send(Err(e));
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

    let (tx, rx) = unbounded_channel::<Result<(), HalStorageOperationErr>>();

    sender.send(HalStorageOperation::Read {
        buffer,
        lba,
        sender: tx,
    });

    if let Some(res) = rx.recv().await {
        res
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

    let (tx, rx) = unbounded_channel::<Result<(), HalStorageOperationErr>>();

    sender.send(HalStorageOperation::Write {
        buffer,
        lba,
        sender: tx,
    });

    if let Some(res) = rx.recv().await {
        res
    } else {
        Err(HalStorageOperationErr::DriveDidntRespond)
    }
}

pub async fn init_gpt(index: usize, force: bool) -> Result<(), GPTErr> {
    let sender = if index == PRIMARY {
        PRIMARY_STORAGE_SENDER.get().unwrap().clone()
    } else {
        SECONDARY_STORAGE_SENDER.get().unwrap().clone()
    };

    let (tx, rx) = unbounded_channel::<Result<(), GPTErr>>();

    sender.send(HalStorageOperation::InitGpt { force, sender: tx });

    if let Some(res) = rx.recv().await {
        res
    } else {
        Err(GPTErr::DriveDidntRespond)
    }
}

pub async fn read_gpt(index: usize) -> Result<(GPTHeader, Vec<GPTEntry>), GPTErr> {
    let sender = if index == PRIMARY {
        PRIMARY_STORAGE_SENDER.get().unwrap().clone()
    } else {
        SECONDARY_STORAGE_SENDER.get().unwrap().clone()
    };

    let (tx, rx) = unbounded_channel::<Result<(GPTHeader, Vec<GPTEntry>), GPTErr>>();

    sender.send(HalStorageOperation::ReadGpt { sender: tx });

    if let Some(res) = rx.recv().await {
        res
    } else {
        Err(GPTErr::DriveDidntRespond)
    }
}

pub async fn add_entry(
    index: usize,
    name: &[u16; 36],
    start_lba: u64,
    end_lba: u64,
    type_guid: Guid,
    flags: u64,
) -> Result<(), GPTErr> {
    let sender = if index == PRIMARY {
        PRIMARY_STORAGE_SENDER.get().unwrap().clone()
    } else {
        SECONDARY_STORAGE_SENDER.get().unwrap().clone()
    };

    let (tx, rx) = unbounded_channel::<Result<(), GPTErr>>();

    // Copy the provided name into a heap allocation and leak it so the
    // reference we send through the channel has a 'static lifetime.
    let mut name_arr: [u16; 36] = [0u16; 36];
    name_arr.copy_from_slice(name);
    let boxed_name = Box::new(name_arr);
    let leaked_name: &'static [u16; 36] = Box::leak(boxed_name);

    sender.send(HalStorageOperation::AddEntry {
        name: leaked_name,
        start_lba,
        end_lba,
        type_guid,
        flags,
        sender: tx,
    });

    if let Some(res) = rx.recv().await {
        res
    } else {
        Err(GPTErr::DriveDidntRespond)
    }
}

pub async fn delete_entry(index: usize, idx: u32) -> Result<(), GPTErr> {
    let sender = if index == PRIMARY {
        PRIMARY_STORAGE_SENDER.get().unwrap().clone()
    } else {
        SECONDARY_STORAGE_SENDER.get().unwrap().clone()
    };

    let (tx, rx) = unbounded_channel::<Result<(), GPTErr>>();

    sender.send(HalStorageOperation::DeleteEntry { idx, sender: tx });

    if let Some(res) = rx.recv().await {
        res
    } else {
        Err(GPTErr::DriveDidntRespond)
    }
}

#[derive(Debug, Clone, Error)]
pub enum HalStorageOperationErr {
    #[error("Drive didn't respond")]
    DriveDidntRespond,
    #[error("Drive responded with error: {0}")]
    DriveErr(String),
    #[error("Drive doesn't have enough space")]
    NoEnoughSpace,
    #[error("Internal error at {0}, {1}: {2}")]
    Internal(u32, u32, String),
}
