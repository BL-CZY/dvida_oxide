pub mod elf;
pub mod loader;

use ejcineque::sync::spin::SpinMutex;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref CurrentThread: SpinMutex<Option<Thread>> = SpinMutex::new(None);
}

#[derive(Debug)]
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

#[derive(Debug)]
pub struct ThreadState {
    pub registers: GPRegisterState,
    pub stack_pointer: u64,
    pub instruction_pointer: u64,
    /// fs
    pub thread_local_segment: u64,
    /// gs
    pub kernel_structs_segment: u64,
    /// cr3
    pub page_table_pointer: u64,

    pub fpu_registers: Option<FPURegisterState>,
    pub simd_registers: Option<SIMDRegisterState>,
}

#[derive(Debug)]
pub struct Thread {
    pub id: usize,
    pub state: ThreadState,
    pub ticks_left: u64,
}

pub async fn load_thread() {}
