use core::sync::atomic::AtomicU64;
use core::task::Waker;

use crate::drivers::ata::pata::{PATA_PRIMARY_BASE, PATA_SECONDARY_BASE, PataDevice};
use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;
use alloc::collections::vec_deque::VecDeque;
use alloc::vec;
use alloc::vec::Vec;
use ejcineque::sync::mpsc::unbounded::UnboundedReceiver;
use lazy_static::lazy_static;
use spin::Mutex;
use thiserror::Error;

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

pub struct HalStorageDevice {
    pub device_io_type: DeviceType,
    pub device_loc: usize,
    pub available: bool,
    pub pata_port: u16,

    pub current_task_id: Option<AtomicU64>,
    pub task_id_counter: AtomicU64,
    pub task_id_queue: alloc::collections::VecDeque<u64>,
    pub task_map: BTreeMap<u64, HalStorageOperation>,
}

pub enum HalStorageOperation {
    Read {
        buffer: Box<[[u8; BLOCK_SIZE]]>,
        lba: i64,
        waker: Option<Waker>,
    },
}

lazy_static! {
    pub static ref STORAGE_CONTEXT_ARR: Vec<Mutex<HalStorageDevice>> = vec![
        Mutex::new(HalStorageDevice::new(PRIMARY, PATA_PRIMARY_BASE)),
        Mutex::new(HalStorageDevice::new(SECONDARY, PATA_SECONDARY_BASE))
    ];
}

impl HalStorageDevice {
    pub fn new(loc: usize, pata_port: u16) -> Self {
        HalStorageDevice {
            device_io_type: DeviceType::Unidentified,
            device_loc: loc,
            available: false,
            pata_port,
            current_task_id: None,
            task_id_counter: AtomicU64::new(0),
            task_id_queue: VecDeque::new(),
            task_map: BTreeMap::new(),
        }
    }

    pub async fn run(&mut self, rx: UnboundedReceiver<HalStorageOperation>) {
        while let Some(op) = rx.recv().await {
            match op {
                HalStorageOperation::Read { buffer, lba, waker } => {}
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
}
