use core::fmt::Debug;
use core::pin::Pin;

use crate::arch::x86_64::pcie::{
    MassStorageControllerSubClass, PciBaseClass, PciDevice, SataProgIf,
};
use crate::crypto::guid::Guid;
use crate::drivers::ata::pata::PataDevice;
use crate::drivers::ata::sata::AhciSata;
use crate::drivers::ata::sata::ahci::AhciHba;
use crate::drivers::ata::sata::task::CUR_AHCI_IDX;
use crate::ejcineque::sync::mpsc::unbounded::{
    UnboundedReceiver, UnboundedSender, unbounded_channel,
};
use crate::ejcineque::sync::mutex::Mutex;
use crate::ejcineque::sync::spsc::cell::{SpscCell, SpscCellSetter, spsc_cells};
use crate::hal::buffer::Buffer;
use crate::{SPAWNER, log};
use alloc::collections::btree_map::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use alloc::{boxed::Box, string::String};
use once_cell_no_std::OnceCell;
use thiserror::Error;

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
    pub tx: UnboundedSender<HalStorageOperation>,
    pub rx: UnboundedReceiver<HalStorageOperation>,
    pub device_inner: Arc<Mutex<Box<dyn HalBlockDevice>>>,
}

#[derive(Debug)]
pub struct HalIdentifyData {
    pub sector_count: u64,
    pub sectors_per_track: u16,
}

#[derive(Debug)]
/// TODO: page cache
pub enum HalStorageOperation {
    Read {
        buffer: Buffer,
        lba: i64,
        setter: SpscCellSetter<Result<(), HalStorageOperationErr>>,
    },

    Write {
        buffer: Buffer,
        lba: i64,
        setter: SpscCellSetter<Result<(), HalStorageOperationErr>>,
    },

    Flush {
        setter: SpscCellSetter<Result<(), HalStorageOperationErr>>,
    },

    Identify {
        setter: SpscCellSetter<HalIdentifyData>,
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

    fn run<'device, 'rx, 'future>(
        &'device mut self,
        rx: &'rx UnboundedReceiver<HalStorageOperation>,
    ) -> Pin<Box<dyn Future<Output = ()> + 'future + Send + Sync>>
    where
        'rx: 'future,
        'device: 'future;
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Ord, Eq)]
pub struct StorageDeviceIdx(pub usize);

static STORAGE_DEVICES_BY_IDX: OnceCell<BTreeMap<StorageDeviceIdx, HalStorageDevice>> =
    OnceCell::new();
static STORAGE_DEVICES_BY_GUID: OnceCell<Mutex<BTreeMap<Guid, StorageDeviceIdx>>> = OnceCell::new();

fn get_storage_devices() -> &'static BTreeMap<StorageDeviceIdx, HalStorageDevice> {
    STORAGE_DEVICES_BY_IDX
        .get()
        .expect("Can't get storage array")
}

fn get_storage_devices_by_guid() -> &'static Mutex<BTreeMap<Guid, StorageDeviceIdx>> {
    STORAGE_DEVICES_BY_GUID
        .get()
        .expect("Can't get storage array")
}

impl HalStorageDevice {
    pub fn sata_ahci(sata: AhciSata) -> Self {
        let (tx, rx) = unbounded_channel::<HalStorageOperation>();
        HalStorageDevice {
            tx,
            rx,
            device_inner: Arc::new(Mutex::new(Box::new(sata))),
        }
    }
}

pub async fn get_identify_data(idx: usize) -> Result<HalIdentifyData, HalStorageOperationErr> {
    let sender = get_storage_devices()
        .get(&StorageDeviceIdx(idx))
        .ok_or(HalStorageOperationErr::DriveDidntRespond)?
        .tx
        .clone();

    let (getter, setter) = spsc_cells::<HalIdentifyData>();

    sender.send(HalStorageOperation::Identify { setter });

    Ok(getter.get().await)
}

pub async fn read_sectors_by_guid(
    guid: Guid,
    buffer: Buffer,
    lba: i64,
) -> Result<(), HalStorageOperationErr> {
    read_sectors_by_idx(
        get_storage_devices_by_guid()
            .lock()
            .await
            .get(&guid)
            .ok_or(HalStorageOperationErr::DriveDidntRespond)?
            .0,
        buffer,
        lba,
    )
    .await
}

pub async fn read_sectors_by_idx(
    index: usize,
    buffer: Buffer,
    lba: i64,
) -> Result<(), HalStorageOperationErr> {
    let sender = get_storage_devices()
        .get(&StorageDeviceIdx(index))
        .ok_or(HalStorageOperationErr::DriveDidntRespond)?
        .tx
        .clone();

    let (getter, setter) = spsc_cells::<Result<(), HalStorageOperationErr>>();

    sender.send(HalStorageOperation::Read {
        buffer,
        lba,
        setter,
    });

    getter.get().await
}

pub async fn write_sectors_by_guid(
    guid: Guid,
    buffer: Buffer,
    lba: i64,
) -> Result<(), HalStorageOperationErr> {
    write_sectors_by_idx(
        get_storage_devices_by_guid()
            .lock()
            .await
            .get(&guid)
            .ok_or(HalStorageOperationErr::DriveDidntRespond)?
            .0,
        buffer,
        lba,
    )
    .await
}

pub async fn write_sectors_by_idx(
    index: usize,
    buffer: Buffer,
    lba: i64,
) -> Result<(), HalStorageOperationErr> {
    let sender = get_storage_devices()
        .get(&StorageDeviceIdx(index))
        .ok_or(HalStorageOperationErr::DriveDidntRespond)?
        .tx
        .clone();

    let (getter, setter) = spsc_cells::<Result<(), HalStorageOperationErr>>();

    sender.send(HalStorageOperation::Write {
        buffer,
        lba,
        setter: setter,
    });

    getter.get().await
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
    device_tree: &mut BTreeMap<u8, BTreeMap<u8, BTreeMap<u8, Vec<PciDevice>>>>,
) {
    let mut storage_devices_list: Vec<HalStorageDevice> = Vec::new();

    if let Some(m) = device_tree.get(&(PciBaseClass::MassStorage as u8)) {
        for device in m.values().flatten().map(|(_, b)| b).flatten() {
            if device.header_partial.subclass == MassStorageControllerSubClass::Sata as u8
                && device.header_partial.prog_if == SataProgIf::Ahci as u8
            {
                log!("Initializing AHCI..");
                let idx = CUR_AHCI_IDX.fetch_add(1, core::sync::atomic::Ordering::AcqRel);
                if idx >= 8 {
                    log!("Too many AHCI devices, skipping");
                }

                let mut ahci = AhciHba::new(device.address, idx as usize);

                for device in ahci.init().drain(0..) {
                    let device = HalStorageDevice::sata_ahci(device);
                    storage_devices_list.push(device)
                }
            }
        }
    }

    let mut storage_devices = BTreeMap::new();
    let mut idx = 0;

    for device in storage_devices_list {
        storage_devices.insert(StorageDeviceIdx(idx), device);
        idx += 1;
    }

    let _ = STORAGE_DEVICES_BY_IDX.set(storage_devices);

    log!("Initialized the storage drives");
}

pub async fn run_storage_devices() {
    for device in STORAGE_DEVICES_BY_IDX.get().expect("Rust error") {
        let device_inner = device.1.device_inner.clone();
        SPAWNER
            .get()
            .expect("No spawner")
            .spawn(async move { device_inner.lock().await.run(&device.1.rx).await });
    }
}
