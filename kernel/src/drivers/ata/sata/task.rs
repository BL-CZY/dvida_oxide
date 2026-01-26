use core::{ops::DerefMut, sync::atomic::AtomicU8, task::Waker};

use lazy_static::lazy_static;
use once_cell_no_std::OnceCell;
use x86_64::{
    VirtAddr,
    instructions::interrupts::{self, without_interrupts},
};

use crate::{
    drivers::ata::sata::{
        AhciSata, AhciSataPorts,
        ahci::{AhciHbaPorts, HBA_PORT_PORTS_OFFSET, HBA_PORT_SIZE},
    },
    ejcineque::sync::{mpsc::unbounded::UnboundedReceiver, spin::SpinMutex},
    hal::storage::HalStorageOperation,
};

pub static CUR_AHCI_IDX: AtomicU8 = AtomicU8::new(0x0);

lazy_static! {
    /// max support 8 ahci's
    pub static ref AHCI_WAKERS_MAP: [[[SpinMutex<Option<Waker>>; 32]; 32]; 8] = Default::default();

    // ghc bases
    pub static ref AHCI_PORTS_MAP: [OnceCell<VirtAddr>; 8] = Default::default();
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

fn port_interrupt_handler(hba_idx: usize, hba_base: VirtAddr, port_idx: usize) {
    let base = hba_base + HBA_PORT_PORTS_OFFSET + port_idx as u64 * HBA_PORT_SIZE;
    let ports = AhciSataPorts { base };

    let interrupt_status = ports.read_interrupt_status();
    for i in 0..32 {
        if interrupt_status & (0x1 << i) != 0 {
            let lock = &AHCI_WAKERS_MAP[hba_idx][port_idx][i];
            without_interrupts(|| {
                if let Some(w) = lock.lock().deref_mut().take() {
                    w.wake();
                }
            });
        }
    }
}

pub fn ahci_interrupt_handler_by_idx(idx: usize) {
    let Some(base) = AHCI_PORTS_MAP[idx].get() else {
        return;
    };

    let ports = AhciHbaPorts { base: *base };

    let interrupt_status = ports.read_interrupt_status();

    for i in 0..32 {
        if interrupt_status & (0x1 << i) != 0 {
            port_interrupt_handler(idx, ports.base, i);
        }
    }
}
