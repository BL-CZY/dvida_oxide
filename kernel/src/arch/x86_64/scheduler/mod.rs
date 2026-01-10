use alloc::collections::vec_deque::VecDeque;
use ejcineque::{
    futures::yield_now,
    sync::{mpsc::unbounded::unbounded_channel, spin::SpinMutex},
};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref CurrentThread: SpinMutex<Option<Thread>> = SpinMutex::new(None);
}

#[derive(Debug)]
pub struct ThreadState {}

#[derive(Debug)]
pub struct Thread {
    pub id: usize,
    pub state: ThreadState,
    pub ticks_left: u64,
}

pub struct ThreadFuture {}

pub enum SchedulerCmd {}

pub async fn run_scheduler() {
    let threads: VecDeque<Thread> = VecDeque::new();
    let (tx, rx) = unbounded_channel::<SchedulerCmd>();

    loop {
        yield_now().await;
    }
}
