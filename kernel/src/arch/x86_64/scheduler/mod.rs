pub mod elf;
pub mod loader;
pub mod syscall;

use core::sync::atomic::AtomicUsize;

use alloc::collections::{btree_map::BTreeMap, vec_deque::VecDeque};
use ejcineque::sync::mutex::Mutex;
use lazy_static::lazy_static;
use x86_64::{
    PhysAddr, VirtAddr,
    registers::rflags::{self, RFlags},
};

use crate::{
    EXECUTOR,
    arch::x86_64::{
        memory::{
            frame_allocator::setup_stack_for_kernel_task, get_hhdm_offset,
            page_table::KERNEL_PAGE_TABLE,
        },
        scheduler::syscall::resume_thread,
    },
    hcf,
};

lazy_static! {
    pub static ref CURRENT_THREAD: Mutex<Option<Thread>> = Mutex::new(None);
    pub static ref THREADS: Mutex<VecDeque<Thread>> = Mutex::new(VecDeque::new());
    pub static ref WAITING_QUEUE: Mutex<BTreeMap<usize, Thread>> = Mutex::new(BTreeMap::new());
    pub static ref WAITING_QUEUE_IDX: AtomicUsize = AtomicUsize::new(0);
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
    pub registers: GPRegisterState,
    pub stack_pointer: VirtAddr,
    /// fs
    pub thread_local_segment: VirtAddr,
    /// cr3
    pub page_table_pointer: PhysAddr,

    pub fpu_registers: Option<FPURegisterState>,
    pub simd_registers: Option<SIMDRegisterState>,
    pub state: State,
}

#[derive(Debug, PartialEq, PartialOrd)]
pub enum PrivilageLevel {
    Kernel,
    User,
}

#[derive(Debug)]
pub struct Thread {
    pub id: usize,
    pub state: ThreadState,
    pub privilage_level: PrivilageLevel,
    pub ticks_left: u64,
}

pub const DEFAULT_TICKS_PER_THREAD: u64 = 500;

pub fn load_kernel_thread() -> ! {
    let kernel_task_stack_start = setup_stack_for_kernel_task().as_u64();

    let thread = Thread {
        id: 0,
        state: ThreadState {
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
                instruction_pointer: kernel_thread_entry_point as u64,
                rflags: rflags::read(),
            },
        },
        privilage_level: PrivilageLevel::Kernel,
        ticks_left: DEFAULT_TICKS_PER_THREAD,
    };

    resume_thread(thread);
}

#[unsafe(no_mangle)]
extern "C" fn kernel_thread_entry_point() -> ! {
    EXECUTOR
        .get()
        .expect("Failed to get the executor")
        .spin_acquire_lock()
        .run();

    hcf();
}

pub async fn load_thread() {}
