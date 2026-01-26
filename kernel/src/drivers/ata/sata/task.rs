use core::{ops::DerefMut, sync::atomic::AtomicU8, task::Waker};

use lazy_static::lazy_static;
use once_cell_no_std::OnceCell;
use x86_64::{VirtAddr, instructions::interrupts::without_interrupts};

use crate::{
    drivers::ata::sata::{AhciSata, ahci::AhciHbaPorts},
    ejcineque::{
        self,
        futures::race::Either,
        sync::{mpsc::unbounded::UnboundedReceiver, spin::SpinMutex},
    },
    hal::storage::HalStorageOperation,
};

pub static CUR_AHCI_IDX: AtomicU8 = AtomicU8::new(0x0);

lazy_static! {
    /// max support 8 ahci's
    pub static ref AHCI_WAKERS_MAP: [[SpinMutex<Option<Waker>>; 32]; 8] = Default::default();

    // ghc bases
    pub static ref AHCI_PORTS_MAP: [OnceCell<VirtAddr>; 8] = Default::default();
}

pub struct AhciSataPortFuture {
    pub hba_idx: usize,
    pub port_idx: usize,
    pub awaken: bool,
}

impl AhciSataPortFuture {
    pub fn new(hba_idx: usize, port_idx: usize) -> Self {
        AhciSataPortFuture {
            hba_idx,
            port_idx,
            awaken: false,
        }
    }
}

impl Future for AhciSataPortFuture {
    type Output = ();

    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        if self.awaken {
            core::task::Poll::Ready(())
        } else {
            self.as_mut().awaken = true;
            without_interrupts(|| {
                *AHCI_WAKERS_MAP[self.hba_idx][self.port_idx].lock() = Some(cx.waker().clone());
            });
            core::task::Poll::Pending
        }
    }
}

impl AhciSata {
    fn finish_operation(&mut self, op: HalStorageOperation) {
        match op {
            HalStorageOperation::Read { buffer, sender, .. } => {
                sender.send(Ok(buffer));
            }

            HalStorageOperation::Write { sender, .. } => {
                sender.send(Ok(()));
            }

            _ => {}
        }
    }

    pub async fn run_task(&mut self, rx: &UnboundedReceiver<HalStorageOperation>) {
        let mut operations: [Option<HalStorageOperation>; 32] = Default::default();

        loop {
            let combined_future = ejcineque::futures::race::race(
                rx.recv(),
                AhciSataPortFuture::new(self.hba_idx, self.ports_idx),
            );

            match combined_future.await {
                Either::Left(Some(op)) => {}
                Either::Right(_) => {
                    let interrupt_status = self.ports.read_interrupt_status();
                    for i in 0..32 {
                        if interrupt_status & (0x1 << i) != 0 {
                            if let Some(op) = operations[i].take() {
                                self.finish_operation(op);
                            }
                        }
                    }
                    self.ports.write_interrupt_status(interrupt_status);
                }
                _ => {}
            }
        }
    }
}

fn port_interrupt_handler(hba_idx: usize, port_idx: usize) {
    let lock = &AHCI_WAKERS_MAP[hba_idx][port_idx];
    without_interrupts(|| {
        if let Some(w) = lock.lock().deref_mut().take() {
            w.wake();
        }
    });
}

pub fn ahci_interrupt_handler_by_idx(idx: usize) {
    let Some(base) = AHCI_PORTS_MAP[idx].get() else {
        return;
    };

    let mut ports = AhciHbaPorts { base: *base };

    let interrupt_status = ports.read_interrupt_status();

    for i in 0..32 {
        if interrupt_status & (0x1 << i) != 0 {
            port_interrupt_handler(idx, i);
        }
    }

    ports.write_interrupt_status(interrupt_status);
}
