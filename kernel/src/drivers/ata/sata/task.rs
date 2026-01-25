use core::task::Waker;

use lazy_static::lazy_static;

use crate::{
    drivers::ata::sata::AhciSata,
    ejcineque::sync::{mpsc::unbounded::UnboundedReceiver, spin::SpinMutex},
    hal::storage::HalStorageOperation,
};

lazy_static! {
    /// max support 8 ahci's
    pub static ref AHCI_WAKERS_MAP: [[[SpinMutex<Option<Waker>>; 32]; 32]; 8] = Default::default();
}

impl AhciSata {
    pub async fn run_task(&mut self, rx: &UnboundedReceiver<HalStorageOperation>) {
        loop {
            while let Some(op) = rx.recv().await {
                match op {
                    HalStorageOperation::Read {
                        buffer,
                        lba,
                        sender,
                    } => {}
                    HalStorageOperation::Write {
                        buffer,
                        lba,
                        sender,
                    } => {}

                    _ => {}
                }
            }
        }
    }
}
