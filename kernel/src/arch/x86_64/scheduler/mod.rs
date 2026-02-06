pub mod elf;
pub mod loader;
pub mod syscall;

use alloc::vec;
use core::{sync::atomic::AtomicUsize, time::Duration};

use alloc::{
    collections::{btree_map::BTreeMap, vec_deque::VecDeque},
    vec::Vec,
};
use x86_64::{
    PhysAddr, VirtAddr,
    registers::rflags::{self, RFlags},
    structures::paging::PhysFrame,
};

use crate::{
    EXECUTOR,
    arch::x86_64::{
        memory::{
            frame_allocator::DEALLOCATOR_SENDER, get_hhdm_offset, page_table::KERNEL_PAGE_TABLE,
        },
        scheduler::syscall::resume_thread,
    },
    get_per_cpu_data, get_per_cpu_data_mut, hcf, log,
};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct ThreadId(usize);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct ProcessId(usize);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct CpuCoreId(u32);

pub struct SchedulerContext {
    pub processes: BTreeMap<ProcessId, Vec<(CpuCoreId, ThreadId)>>,
    pub cpu_contexts: Vec<SchedulerCpuContext>,
}

pub static THREAD_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Default)]
pub struct SchedulerCpuContext {
    pub thread_map: BTreeMap<ThreadId, Thread>,
    pub thread_queue: VecDeque<ThreadId>,
    pub current_thread: Option<ThreadId>,
    pub waiting_threads: BTreeMap<usize, ThreadId>,
    pub waiting_queue_idx: usize,
}

impl SchedulerCpuContext {
    // the thread will start with paused status
    pub fn spawn_thread(&mut self, mut thread: Thread) {
        let id = THREAD_ID_COUNTER.fetch_add(1, core::sync::atomic::Ordering::AcqRel);

        thread.id = ThreadId(id);
        self.thread_map.insert(ThreadId(id), thread);
        self.thread_queue.push_back(ThreadId(id));
    }

    pub fn get_current_thread_ref(&mut self) -> &mut Thread {
        let id = self.current_thread.as_ref().expect("No current thread");
        self.thread_map.get_mut(id).expect("Corrupted metadata")
    }

    pub fn switch_task(&mut self) -> &mut Thread {
        loop {
            let id = self.thread_queue.pop_front().expect("KERNEL TASK IS DEAD");

            // remove stale thread
            if let Some(thread) = self.thread_map.get(&id) {
                if thread.state.killed {
                    self.thread_map.remove(&id);
                } else {
                    self.current_thread = Some(id);
                    return self.thread_map.get_mut(&id).expect("Rust error");
                }
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct GPRegisterState {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
}

#[derive(Debug)]
pub struct FPURegisterState {}

#[derive(Debug)]
pub struct SIMDRegisterState {}

#[derive(Debug, PartialEq)]
pub enum State {
    Paused {
        instruction_pointer: u64,
        rflags: RFlags,
    },
    Waiting,
    Ready,
}

#[derive(Debug)]
pub struct ThreadState {
    pub killed: bool,

    pub registers: GPRegisterState,
    pub stack_pointer: VirtAddr,
    /// fs
    pub thread_local_segment: VirtAddr,
    /// cr3
    pub page_table_pointer: PhysAddr,

    pub fpu_registers: Option<FPURegisterState>,
    pub simd_registers: Option<SIMDRegisterState>,
    pub state: State,

    pub frames: Vec<PhysFrame>,
}

#[derive(Debug, PartialEq, PartialOrd)]
pub enum PrivilageLevel {
    Kernel,
    User,
}

#[derive(Debug)]
pub struct Thread {
    pub id: ThreadId,
    pub state: ThreadState,
    pub privilage_level: PrivilageLevel,
    pub time_left: Duration,
}

impl Drop for Thread {
    fn drop(&mut self) {
        let frames_to_free = core::mem::take(&mut self.state.frames);

        DEALLOCATOR_SENDER
            .get()
            .expect("Failed to get deallocator sender")
            .send(frames_to_free);
    }
}

pub const DEFAULT_TICKS_PER_THREAD: Duration = Duration::from_millis(5);

pub fn load_kernel_thread() -> ! {
    let per_cpu_data = get_per_cpu_data_mut!();
    let kernel_task_stack_start = per_cpu_data.kernel_task_stack_ptr;

    let thread = Thread {
        id: ThreadId(0),
        state: ThreadState {
            killed: false,
            registers: GPRegisterState::default(),
            stack_pointer: VirtAddr::new(kernel_task_stack_start),
            // kernel doesn't have a thread local segment
            thread_local_segment: VirtAddr::new(0),
            page_table_pointer: PhysAddr::new(
                KERNEL_PAGE_TABLE
                    .get()
                    .expect("Failed to get kernel page table")
                    .spin_acquire_lock()
                    .table_ptr as u64
                    - get_hhdm_offset().as_u64(),
            ),
            fpu_registers: None,
            simd_registers: None,
            state: State::Paused {
                instruction_pointer: kernel_thread_entry_point as *const () as u64,
                rflags: rflags::read(),
            },

            // if the kernel dies no need to deallocate
            frames: vec![],
        },
        privilage_level: PrivilageLevel::Kernel,
        time_left: DEFAULT_TICKS_PER_THREAD,
    };

    resume_thread(&thread);
}

#[unsafe(no_mangle)]
extern "C" fn kernel_thread_entry_point() -> ! {
    let id = get_per_cpu_data!().id as u32;

    if let Some(ctx) = EXECUTOR
        .get()
        .expect("Failed to get the executor")
        .contexts
        .get(&id)
    {
        log!("Starting kernel task");
        ctx.run();
    }

    log!("Didn't find context for core");

    hcf();
}

pub async fn load_thread() {}
