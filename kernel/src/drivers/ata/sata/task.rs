use core::{ops::DerefMut, sync::atomic::AtomicU8, task::Waker};

use lazy_static::lazy_static;
use once_cell_no_std::OnceCell;
use x86_64::{VirtAddr, instructions::interrupts::without_interrupts};

use crate::{
    drivers::ata::sata::{AhciSata, PortInterruptStatus, PortTaskFileData, ahci::AhciHbaPorts},
    ejcineque::{
        self,
        futures::race::Either,
        sync::{mpsc::unbounded::UnboundedReceiver, spin::SpinMutex},
    },
    hal::storage::HalStorageOperation,
    log,
};

pub static CUR_AHCI_IDX: AtomicU8 = AtomicU8::new(0x0);

lazy_static! {
    /// max support 8 ahci's
    pub static ref AHCI_WAKERS_MAP: [[SpinMutex<Option<Waker>>; 32]; 8] = Default::default();

    // ghc bases
    pub static ref AHCI_PORTS_MAP: [OnceCell<VirtAddr>; 8] = Default::default();
}

#[derive(Debug)]
pub struct AhciTaskState {
    pub operations: [Option<HalStorageOperation>; 32],
    pub remaining_operations: u64,
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
    fn finish_operation(
        &mut self,
        op: HalStorageOperation,
        is_err: bool,
        state: &mut AhciTaskState,
    ) {
        match op {
            HalStorageOperation::Read { buffer, sender, .. } => {
                if is_err {
                    sender.send(Err(crate::hal::storage::HalStorageOperationErr::DriveErr(
                        "".into(),
                    )));
                }

                sender.send(Ok(buffer));
            }

            HalStorageOperation::Write { sender, .. } => {
                sender.send(Ok(()));

                if is_err {
                    sender.send(Err(crate::hal::storage::HalStorageOperationErr::DriveErr(
                        "".into(),
                    )));
                }
            }

            _ => {}
        }

        state.remaining_operations += 1;
    }

    async fn handle_interrupt(&mut self, state: &mut AhciTaskState) {
        let mut error = false;
        let interrupt_status = PortInterruptStatus(self.ports.read_interrupt_status());
        if interrupt_status.interface_fatal_error() || interrupt_status.host_bus_fatal_error() {
            // TODO: set everything to failure
            self.failure_reset().await;
        }

        if interrupt_status.interface_non_fatal_error() {
            todo!();
        }

        if interrupt_status.host_bus_data_error() {
            todo!();
        }

        if interrupt_status.task_file_error() {
            error = true;
            log!(
                "Error from AHCI SATA: {:b}",
                PortTaskFileData(self.ports.read_task_file_data()).error_code()
            )
        }

        let cmd_issue = self.ports.read_command_issue();
        for i in 0..32 {
            if cmd_issue & (0x1 << i) == 0 && state.operations[i].is_some() {
                if let Some(op) = state.operations[i].take() {
                    self.finish_operation(op, error, state);
                }
            }
        }

        self.ports.write_command_issue(cmd_issue);
        self.ports.write_interrupt_status(interrupt_status.0);
    }

    async fn launch_operation(
        &mut self,
        i: usize,
        op: HalStorageOperation,
        state: &mut AhciTaskState,
    ) {
        match &op {
            HalStorageOperation::Read { buffer, lba, .. } => {
                self.start_read_sectors(i, *lba, buffer.clone()).await;
            }

            HalStorageOperation::Write { buffer, lba, .. } => {
                self.start_write_sectors(i, *lba, buffer.clone()).await;
            }

            _ => {}
        }

        state.operations[i] = Some(op);
    }

    async fn start_operation(&mut self, op: HalStorageOperation, state: &mut AhciTaskState) {
        state.remaining_operations -= 1;

        for i in 0..=self.max_cmd_slots as usize {
            if state.operations[i].is_none() {
                self.launch_operation(i, op, state).await;

                break;
            }
        }
    }

    pub async fn run_task(&mut self, rx: &UnboundedReceiver<HalStorageOperation>) {
        let operations: [Option<HalStorageOperation>; 32] = Default::default();
        let remaining_operations = self.max_cmd_slots + 1;

        let mut state = AhciTaskState {
            operations,
            remaining_operations,
        };

        loop {
            let sata_future = AhciSataPortFuture::new(self.hba_idx, self.ports_idx);

            if remaining_operations > 0 {
                let combined_future = ejcineque::futures::race::race(rx.recv(), sata_future);

                match combined_future.await {
                    Either::Left(Some(op)) => {
                        self.start_operation(op, &mut state).await;
                    }
                    Either::Right(_) => {
                        self.handle_interrupt(&mut state).await;
                    }
                    _ => {}
                }
            } else {
                sata_future.await;
                self.handle_interrupt(&mut state).await;
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
