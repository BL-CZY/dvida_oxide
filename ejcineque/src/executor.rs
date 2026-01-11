use alloc::boxed::Box;
use alloc::collections::{BTreeMap, vec_deque::VecDeque};
use alloc::sync::Arc;
use alloc::task::Wake;
// use terminal::{iprint, iprintln, log};

use core::arch::asm;
use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::AtomicU64;
use core::task::{Context, Poll, Waker};
use spin::Mutex;

#[derive(Debug, Clone, Copy, Ord, PartialEq, Eq, PartialOrd)]
pub struct TaskID(u64);

pub struct Task {
    pub id: TaskID,
    pub future: Pin<Box<dyn Future<Output = ()> + Send>>,
}

impl Task {
    pub fn poll(&mut self, ctx: &mut Context) -> Poll<()> {
        self.future.as_mut().poll(ctx)
    }
}

#[derive(Debug, Clone)]
pub struct TaskWaker {
    pub id: TaskID,
    pub tasks: Arc<Mutex<VecDeque<TaskID>>>,
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.tasks.lock().push_back(self.id);
    }
}

#[derive(Clone)]
pub struct Spawner {
    pub counter: Arc<AtomicU64>,
    pub tasks: Arc<Mutex<VecDeque<TaskID>>>,
    pub tasks_map: Arc<Mutex<BTreeMap<TaskID, Arc<Mutex<Task>>>>>,
}

impl Spawner {
    pub fn spawn(&self, future: impl Future<Output = ()> + 'static + Send) {
        let future = Box::pin(future);

        // Get ID and increment counter atomically, then release lock
        let id = {
            let id = TaskID(self.counter.load(core::sync::atomic::Ordering::SeqCst));

            if self.counter.load(core::sync::atomic::Ordering::SeqCst) == u64::MAX {
                self.counter.swap(0, core::sync::atomic::Ordering::AcqRel);
            } else {
                self.counter
                    .swap(id.0 + 1, core::sync::atomic::Ordering::AcqRel);
            }

            id // Lock is dropped here
        };

        let task = Task { id, future };

        self.tasks.lock().push_back(id);
        self.tasks_map.lock().insert(id, Arc::new(Mutex::new(task)));
    }
}

#[derive(Default, Clone)]
pub struct Executor {
    pub counter: Arc<AtomicU64>,
    pub tasks: Arc<Mutex<VecDeque<TaskID>>>,
    pub tasks_map: Arc<Mutex<BTreeMap<TaskID, Arc<Mutex<Task>>>>>,
    pub wakers: Arc<Mutex<BTreeMap<TaskID, Arc<TaskWaker>>>>,
}

impl Executor {
    pub fn spawner(&self) -> Spawner {
        Spawner {
            counter: self.counter.clone(),
            tasks: self.tasks.clone(),
            tasks_map: self.tasks_map.clone(),
        }
    }

    pub fn new() -> Self {
        Executor {
            counter: Arc::new(0.into()),
            ..Default::default()
        }
    }

    pub fn spawn(&self, future: impl Future<Output = ()> + 'static + Send) {
        let future = Box::pin(future);

        // Get ID and increment counter atomically, then release lock
        let id = {
            let id = TaskID(self.counter.load(core::sync::atomic::Ordering::SeqCst));

            if self.counter.load(core::sync::atomic::Ordering::SeqCst) == u64::MAX {
                self.counter.swap(0, core::sync::atomic::Ordering::AcqRel);
            } else {
                self.counter
                    .swap(id.0 + 1, core::sync::atomic::Ordering::AcqRel);
            }

            id // Lock is dropped here
        };

        let task = Task { id, future };

        self.tasks.lock().push_back(id);
        self.tasks_map.lock().insert(id, Arc::new(Mutex::new(task)));
    }

    pub fn run(&self) {
        loop {
            // halt when nothing happens
            while self.tasks.lock().is_empty() {
                unsafe {
                    asm!("hlt");
                }
            }

            let id = match self.tasks.lock().pop_front() {
                Some(i) => i,
                None => continue,
            };

            let task = match self.tasks_map.lock().get_mut(&id) {
                Some(t) => t.clone(),
                None => continue,
            };

            let waker = self
                .wakers
                .lock()
                .entry(id)
                .or_insert_with(|| {
                    Arc::new(TaskWaker {
                        id,
                        tasks: self.tasks.clone(),
                    })
                })
                .clone();

            let waker = Waker::from(waker);

            let mut ctx = Context::from_waker(&waker);
            match task.lock().poll(&mut ctx) {
                Poll::Ready(_) => {
                    // the task is finished, remove it
                    self.tasks_map.lock().remove(&id);
                    self.wakers.lock().remove(&id);
                }
                Poll::Pending => {}
            }
        }
    }
}
