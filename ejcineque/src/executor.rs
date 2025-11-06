use alloc::boxed::Box;
use alloc::collections::{BTreeMap, vec_deque::VecDeque};
use alloc::sync::Arc;
use alloc::task::Wake;

use core::arch::asm;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use spin::Mutex;

#[derive(Debug, Clone, Copy, Ord, PartialEq, Eq, PartialOrd)]
pub struct TaskID(u64);

pub struct Task {
    pub id: TaskID,
    pub future: Pin<Box<dyn Future<Output = ()>>>,
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

#[derive(Default)]
pub struct Executor {
    pub counter: Arc<Mutex<u64>>,
    pub tasks: Arc<Mutex<VecDeque<TaskID>>>,
    pub tasks_map: Arc<Mutex<BTreeMap<TaskID, Task>>>,
    pub wakers: Arc<Mutex<BTreeMap<TaskID, Arc<TaskWaker>>>>,
}

impl Executor {
    pub fn new() -> Self {
        Executor {
            counter: Arc::new(Mutex::new(0)),
            ..Default::default()
        }
    }

    pub fn spawn(&self, future: impl Future<Output = ()> + 'static) {
        let future = Box::pin(future);

        let id = TaskID(*self.counter.lock());
        let task = Task { id, future };

        let mut lock_handle = self.counter.lock();

        if *lock_handle == u64::MAX {
            *lock_handle = 0;
        } else {
            *lock_handle += 1;
        }

        self.tasks.lock().push_back(id);
        self.tasks_map.lock().insert(id, task);
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

            let mut task_map_handle = self.tasks_map.lock();
            let task = match task_map_handle.get_mut(&id) {
                Some(t) => t,
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
            match task.poll(&mut ctx) {
                Poll::Ready(_) => {
                    // the task is finished, remove it
                    task_map_handle.remove(&id);
                    self.wakers.lock().remove(&id);
                }
                Poll::Pending => {}
            }
        }
    }
}
