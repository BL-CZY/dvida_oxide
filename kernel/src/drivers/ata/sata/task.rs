use core::{ops::DerefMut, sync::atomic::AtomicU8, task::Waker};

use alloc::string::ToString;
use lazy_static::lazy_static;
use once_cell_no_std::OnceCell;
use thiserror::Error;
use x86_64::{VirtAddr, instructions::interrupts::without_interrupts};

use crate::{
    drivers::ata::sata::{
        AhciSata, AhciSataPorts, AtaError, PortCmdAndStatus, PortInterruptStatus, PortSataError,
        PortTaskFileData,
        ahci::{AhciHbaPorts, HBA_PORT_PORTS_OFFSET, HBA_PORT_SIZE},
    },
    ejcineque::{
        self,
        futures::race::Either,
        sync::{mpsc::unbounded::UnboundedReceiver, spin::SpinMutex},
    },
    hal::storage::{HalIdentifyData, HalStorageOperation},
    log,
};

pub static CUR_AHCI_IDX: AtomicU8 = AtomicU8::new(0x0);

lazy_static! {
    /// max support 8 ahci's
    pub static ref AHCI_WAKERS_MAP: [[SpinMutex<(AhciSataInterruptData, Option<Waker>)>; 32]; 8] = Default::default();

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
    type Output = AhciSataInterruptData;

    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        if self.awaken {
            core::task::Poll::Ready(AHCI_WAKERS_MAP[self.hba_idx][self.port_idx].lock().0)
        } else {
            self.as_mut().awaken = true;
            without_interrupts(|| {
                AHCI_WAKERS_MAP[self.hba_idx][self.port_idx].lock().1 = Some(cx.waker().clone());
            });
            core::task::Poll::Pending
        }
    }
}

#[derive(Error, Debug)]
pub enum AhciErr {
    #[error("{:#?}", 0)]
    ATA(AtaError),
    #[error("Internal drive error")]
    Internal,
}

impl AhciSata {
    fn finish_operation(
        &mut self,
        op: HalStorageOperation,
        err: Option<AhciErr>,
        state: &mut AhciTaskState,
    ) {
        match op {
            HalStorageOperation::Read { setter, .. } => {
                if err.is_some() {
                    setter.set(Err(crate::hal::storage::HalStorageOperationErr::DriveErr(
                        err.unwrap().to_string(),
                    )));
                } else {
                    setter.set(Ok(()));
                }
            }

            HalStorageOperation::Write { setter, .. } => {
                if err.is_some() {
                    setter.set(Err(crate::hal::storage::HalStorageOperationErr::DriveErr(
                        err.unwrap().to_string(),
                    )));
                } else {
                    setter.set(Ok(()));
                }
            }

            HalStorageOperation::Flush { setter } => {
                if err.is_some() {
                    setter.set(Err(crate::hal::storage::HalStorageOperationErr::DriveErr(
                        err.unwrap().to_string(),
                    )));
                } else {
                    setter.set(Ok(()));
                }
            }

            _ => {}
        }

        state.remaining_operations += 1;
    }

    async fn handle_interrupt(&mut self, state: &mut AhciTaskState, data: AhciSataInterruptData) {
        let cmd_issue = self.ports.read_command_issue();
        let interrupt_status = data.interrupt_status;
        if interrupt_status.interface_fatal_error() || interrupt_status.host_bus_fatal_error() {
            for i in 0..32 {
                if let Some(op) = state.operations[i].take() {
                    self.finish_operation(op, Some(AhciErr::Internal), state);
                }
            }

            self.failure_reset().await;

            return;
        }

        if interrupt_status.interface_non_fatal_error() {
            for i in 0..32 {
                if let Some(op) = state.operations[i].take() {
                    self.finish_operation(op, Some(AhciErr::Internal), state);
                }
            }

            log!("interface non fatal error");
            self.com_reset().await;
        }

        if interrupt_status.host_bus_data_error() {
            for i in 0..32 {
                if let Some(op) = state.operations[i].take() {
                    self.finish_operation(op, Some(AhciErr::Internal), state);
                }
            }

            log!("host bus data error");
            self.com_reset().await;
        }

        if interrupt_status.task_file_error() {
            // ST was closed in the interrupt handler earlier so now wait for cmd list to
            // stop
            loop {
                let cmd_and_status = PortCmdAndStatus(self.ports.read_command_and_status());
                if cmd_and_status.cmd_list_running() {
                    core::hint::spin_loop();
                } else {
                    break;
                }
            }

            self.ports.write_sata_error(0xFFFFFFFF);
            self.ports.write_interrupt_status(0xFFFFFFFF);

            let cur_cmd_slot = data.cmd_and_status.cur_cmd_slot();
            if let Some(op) = state.operations[cur_cmd_slot as usize].take() {
                self.finish_operation(
                    op,
                    Some(AhciErr::ATA(AtaError(
                        data.task_file_data.error_code() as u8
                    ))),
                    state,
                );
            }

            // restart
            let mut cmd_and_status = PortCmdAndStatus(self.ports.read_command_and_status());
            cmd_and_status.set_start(true);
            self.ports.write_command_and_status(cmd_and_status.0);

            loop {
                let cmd_and_status = PortCmdAndStatus(self.ports.read_command_and_status());
                if !cmd_and_status.cmd_list_running() {
                    core::hint::spin_loop();
                } else {
                    break;
                }
            }

            // recover the rest of the commands
            let cmd_issue = data.command_issue & !(0x1 << cur_cmd_slot);
            self.ports.write_command_issue(cmd_issue);

            return;
        }

        for i in 0..32 {
            if cmd_issue & (0x1 << i) == 0
                && state.operations[i].is_some()
                && let Some(op) = state.operations[i].take()
            {
                self.finish_operation(op, None, state);
            }
        }

        // self.ports.write_command_issue(cmd_issue);
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

            HalStorageOperation::Flush { .. } => {
                self.issue_flush(i).await;
            }

            _ => {}
        }

        state.operations[i] = Some(op);
    }

    async fn start_operation(&mut self, op: HalStorageOperation, state: &mut AhciTaskState) {
        if let HalStorageOperation::Identify { setter } = op {
            setter.set(HalIdentifyData {
                sectors_per_track: self.identify_data.sectors_per_track,
                sector_count: self.identify_data.lba48_sectors,
            });
            return;
        }

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
                    Either::Right(data) => {
                        self.handle_interrupt(&mut state, data).await;
                    }
                    _ => {}
                }
            } else {
                let data = sata_future.await;
                self.handle_interrupt(&mut state, data).await;
            }
        }
    }
}

fn port_interrupt_handler(hba_idx: usize, port_idx: usize, hba_base: VirtAddr) {
    let mut ports = AhciSataPorts {
        base: hba_base + HBA_PORT_PORTS_OFFSET + HBA_PORT_SIZE * port_idx as u64,
    };

    let info = AhciSataInterruptData {
        interrupt_status: PortInterruptStatus(ports.read_interrupt_status()),
        sata_error: PortSataError(ports.read_sata_error()),
        task_file_data: PortTaskFileData(ports.read_task_file_data()),
        command_issue: ports.read_command_issue(),
        cmd_and_status: PortCmdAndStatus(ports.read_command_and_status()),
    };

    log!("{:#?}", info);

    // stops the dma engine at task file error
    if info.interrupt_status.task_file_error() {
        let mut cmd_and_status = PortCmdAndStatus(ports.read_command_and_status());
        cmd_and_status.set_start(false);
        ports.write_command_and_status(cmd_and_status.0);
    }

    without_interrupts(|| {
        let mut guard = AHCI_WAKERS_MAP[hba_idx][port_idx].lock();
        let (inf, waker) = guard.deref_mut();
        if let Some(w) = waker.take() {
            *inf = info;
            w.wake();
        }
    });

    ports.write_interrupt_status(ports.read_interrupt_status());
    ports.write_sata_error(0xFFFFFFFF);
}

pub fn ahci_interrupt_handler_by_idx(idx: usize) {
    let Some(base) = AHCI_PORTS_MAP[idx].get() else {
        return;
    };

    let mut ports = AhciHbaPorts { base: *base };

    let interrupt_status = ports.read_interrupt_status();

    for i in 0..32 {
        if interrupt_status & (0x1 << i) != 0 {
            port_interrupt_handler(idx, i, ports.base);
        }
    }

    ports.write_interrupt_status(ports.read_interrupt_status());
}

#[derive(Debug, Default, Clone, Copy)]
pub struct AhciSataInterruptData {
    pub command_issue: u32,
    pub cmd_and_status: PortCmdAndStatus,
    pub interrupt_status: PortInterruptStatus,
    pub task_file_data: PortTaskFileData,
    pub sata_error: PortSataError,
}
