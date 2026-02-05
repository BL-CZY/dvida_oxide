use alloc::boxed::Box;
use alloc::collections::{BTreeMap, vec_deque::VecDeque};
use alloc::sync::Arc;
use alloc::task::Wake;
use limine::mp::Cpu;
use x86_64::instructions::interrupts::without_interrupts;

use crate::log;

use super::sync::spin::SpinMutex as Mutex;
use core::arch::asm;
use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::AtomicU64;
use core::task::{Context, Poll, Waker};

#[derive(Debug, Clone, Copy, Ord, PartialEq, Eq, PartialOrd)]
pub struct TaskID(u64);

pub struct Task {
    pub id: TaskID,
    // they stay in the same core to keep cacheline efficiency
    pub queue_id: u32,
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
    pub contexts: Arc<BTreeMap<u32, ExecutorContext>>,
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

        // load balancing
        let queue_id = *self
            .contexts
            .iter()
            .min_by(|(_, val), (_, val1)| {
                x86_64::instructions::interrupts::without_interrupts(|| {
                    val.tasks.lock().len().cmp(&val1.tasks.lock().len())
                })
            })
            .expect("No context")
            .0;

        let task = Task {
            id,
            future,
            queue_id,
        };

        x86_64::instructions::interrupts::without_interrupts(|| {
            self.contexts
                .get(&task.queue_id)
                .expect("Internal runtime error")
                .tasks
                .lock()
                .push_back(id);

            self.contexts
                .get(&task.queue_id)
                .expect("Internal runtime error")
                .tasks_map
                .lock()
                .insert(id, Arc::new(Mutex::new(task)));
        });
    }
}

#[derive(Default, Clone)]
pub struct ExecutorContext {
    pub tasks: Arc<Mutex<VecDeque<TaskID>>>,
    pub tasks_map: Arc<Mutex<BTreeMap<TaskID, Arc<Mutex<Task>>>>>,
    pub wakers: Arc<Mutex<BTreeMap<TaskID, Arc<TaskWaker>>>>,
}

impl ExecutorContext {
    pub fn run(&self) {
        loop {
            // halt when nothing happens
            loop {
                let is_empty = without_interrupts(|| {
                    let is_empty = self.tasks.lock().is_empty();
                    is_empty
                });

                if !is_empty {
                    break;
                }
                unsafe {
                    asm!("hlt");
                }
            }

            use x86_64::registers::rflags::RFlags;

            let flags = x86_64::registers::rflags::read();
            let enabled = flags.contains(RFlags::INTERRUPT_FLAG);

            log!("is enabled: {enabled}");

            let id = match without_interrupts(|| self.tasks.lock().pop_front()) {
                Some(i) => i,
                None => continue,
            };

            let mut task = None;

            without_interrupts(|| {
                task = self
                    .tasks_map
                    .lock()
                    .get_mut(&id)
                    .map_or(None, |v| Some(v.clone()));
            });

            let task = match task {
                Some(t) => t,
                None => continue,
            };

            let waker = without_interrupts(|| {
                self.wakers
                    .lock()
                    .entry(id)
                    .or_insert_with(|| {
                        Arc::new(TaskWaker {
                            id,
                            tasks: self.tasks.clone(),
                        })
                    })
                    .clone()
            });

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

#[derive(Default, Clone)]
pub struct Executor {
    pub counter: Arc<AtomicU64>,
    pub contexts: Arc<BTreeMap<u32, ExecutorContext>>,
}

impl Executor {
    pub fn spawner(&self) -> Spawner {
        Spawner {
            counter: self.counter.clone(),
            contexts: self.contexts.clone(),
        }
    }

    pub fn new(cpus: &[&Cpu]) -> Self {
        let mut contexts = BTreeMap::new();

        for cpu in cpus.iter() {
            contexts.insert(
                cpu.id,
                ExecutorContext {
                    ..Default::default()
                },
            );
        }

        Executor {
            counter: Arc::new(0.into()),
            contexts: contexts.into(),
        }
    }
}
