use core::fmt::Debug;
use core::pin::Pin;

use crate::arch::x86_64::pcie::{
    MassStorageControllerSubClass, PciBaseClass, PciDevice, SataProgIf,
};
use crate::crypto::guid::Guid;
use crate::drivers::ata::pata::{PATA_PRIMARY_BASE, PATA_SECONDARY_BASE, PataDevice};
use crate::drivers::ata::sata::AhciSata;
use crate::drivers::ata::sata::ahci::AhciHba;
use crate::hal::buffer::Buffer;
use crate::hal::gpt::{GPTEntry, GPTErr, GPTHeader};
use alloc::collections::btree_map::BTreeMap;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use alloc::{boxed::Box, string::String};
use ejcineque::sync::mpsc::unbounded::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use ejcineque::sync::mutex::Mutex;
use lazy_static::lazy_static;
use once_cell_no_std::OnceCell;
use terminal::{iprintln, log};
use thiserror::Error;
use x86_64::VirtAddr;

pub static PRIMARY_STORAGE_SENDER: OnceCell<UnboundedSender<HalStorageOperation>> = OnceCell::new();
pub static SECONDARY_STORAGE_SENDER: OnceCell<UnboundedSender<HalStorageOperation>> =
    OnceCell::new();

#[derive(Debug)]
pub enum DeviceType {
    Unidentified,
    PataPio(PataDevice),
    PataDma,
    SataAhci(AhciHba),
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
    pub available: bool,
    pub pata_port: u16,
    pub device_inner: Box<dyn HalBlockDevice>,
}

#[derive(Debug)]
pub enum HalStorageOperation {
    Read {
        buffer: Buffer,
        lba: i64,
        sender: UnboundedSender<Result<Buffer, HalStorageOperationErr>>,
    },

    Write {
        buffer: Buffer,
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
        name: [u16; 36],
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

pub trait HalBlockDevice: Send + Sync + Debug {
    fn sector_count(&mut self) -> u64;
    fn sectors_per_track(&mut self) -> u16;

    fn read_sectors_async(
        &mut self,
        index: i64,
        count: u16,
        output: &mut [u8],
    ) -> Pin<
        Box<
            dyn Future<Output = Result<(), Box<dyn core::error::Error + Send + Sync>>>
                + Send
                + Sync,
        >,
    >;

    fn write_sectors_async(
        &mut self,
        index: i64,
        count: u16,
        input: &[u8],
    ) -> Pin<
        Box<
            dyn Future<Output = Result<(), Box<dyn core::error::Error + Send + Sync>>>
                + Send
                + Sync,
        >,
    >;

    fn init(&mut self) -> Result<(), alloc::boxed::Box<dyn core::error::Error + Send + Sync>>;
}

static STORAGE_DEVICES: OnceCell<Vec<Mutex<HalStorageDevice>>> = OnceCell::new();

fn get_storage_devices() -> &'static Vec<Mutex<HalStorageDevice>> {
    STORAGE_DEVICES.get().expect("Can't get storage array")
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
    get_storage_devices()[index].lock().await.run(rx).await;
}

impl HalStorageDevice {
    pub fn sata_ahci(sata: AhciSata) -> Self {
        HalStorageDevice {
            device_inner: Box::new(sata),
            available: true,
            pata_port: 0,
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
            match op {
                HalStorageOperation::Read {
                    mut buffer,
                    lba,
                    sender,
                } => {
                    match self
                        .read_sectors_async(lba, (buffer.len() / SECTOR_SIZE) as u16, &mut buffer)
                        .await
                    {
                        Ok(_) => {
                            sender.send(Ok(buffer));
                        }
                        Err(e) => {
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
                            sender.send(Ok(()));
                        }

                        Err(e) => {
                            sender.send(Err(HalStorageOperationErr::DriveErr(e.to_string())));
                        }
                    }
                }

                HalStorageOperation::InitGpt { force, sender } => {
                    match self.create_gpt(force).await {
                        Ok(_) => {
                            sender.send(Ok(()));
                        }

                        Err(e) => {
                            sender.send(Err(e));
                        }
                    }
                }

                HalStorageOperation::ReadGpt { sender } => match self.read_gpt().await {
                    Ok(res) => {
                        sender.send(Ok(res));
                    }

                    Err(e) => {
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
                    Ok(_res) => {
                        sender.send(Ok(()));
                    }

                    Err(e) => {
                        sender.send(Err(e));
                    }
                },

                HalStorageOperation::DeleteEntry { idx, sender } => {
                    match self.delete_entry(idx).await {
                        Ok(res) => {
                            sender.send(Ok(res));
                        }

                        Err(e) => {
                            sender.send(Err(e));
                        }
                    }
                }
            }
        }
    }

    pub fn init(&mut self) -> Result<(), Box<dyn core::error::Error + Send + Sync>> {
        Ok(self.device_inner.init()?)
    }

    pub fn sector_count(&mut self) -> u64 {
        self.device_inner.sector_count()
    }

    pub fn sectors_per_track(&mut self) -> u16 {
        self.device_inner.sectors_per_track()
    }

    pub async fn read_sectors_async(
        &mut self,
        index: i64,
        count: u16,
        output: &mut [u8],
    ) -> Result<(), Box<dyn core::error::Error + Send + Sync>> {
        Ok(self
            .device_inner
            .read_sectors_async(index, count, output)
            .await?)
    }

    pub async fn write_sectors_async(
        &mut self,
        index: i64,
        count: u16,
        input: &[u8],
    ) -> Result<(), Box<dyn core::error::Error + Send + Sync>> {
        Ok(self
            .device_inner
            .write_sectors_async(index, count, input)
            .await?)
    }
}

pub async fn read_sectors(
    index: usize,
    buffer: Buffer,
    lba: i64,
) -> Result<Buffer, HalStorageOperationErr> {
    let sender = if index == PRIMARY {
        PRIMARY_STORAGE_SENDER.get().unwrap().clone()
    } else {
        SECONDARY_STORAGE_SENDER.get().unwrap().clone()
    };

    let (tx, rx) = unbounded_channel::<Result<Buffer, HalStorageOperationErr>>();

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
    buffer: Buffer,
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
        name: *leaked_name,
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

pub fn identify_storage_devices(
    device_tree: &mut BTreeMap<u8, BTreeMap<u8, BTreeMap<u8, PciDevice>>>,
) {
    let mut storage_devices: Vec<Mutex<HalStorageDevice>> = Vec::new();

    if let Some(m) = device_tree.get(&(PciBaseClass::MassStorage as u8)) {
        for (_, device) in m.values().flatten() {
            if device.header_partial.subclass == MassStorageControllerSubClass::Sata as u8
                && device.header_partial.prog_if == SataProgIf::Ahci as u8
            {
                let mut ahci = AhciHba::new(device.address);

                for device in ahci.init().drain(0..) {
                    let mut device = HalStorageDevice::sata_ahci(device);
                    match device.init() {
                        Ok(_) => storage_devices.push(Mutex::new(device)),
                        Err(e) => {
                            log!("Failed to initialize Sata Ahci: {}", e);
                        }
                    }
                }
            }
        }
    }

    let _ = STORAGE_DEVICES.set(storage_devices);

    log!("Initialized the storage drives");
}
