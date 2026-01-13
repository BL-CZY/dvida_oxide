use core::arch::{asm, global_asm};

use x86_64::{VirtAddr, registers::model_specific::Msr};

use crate::arch::x86_64::{
    gdt::{KERNEL_CODE_SEGMENT_IDX, USER_CODE_SEGMENT_IDX},
    memory::{PAGE_SIZE, frame_allocator::setup_stack},
};

//TODO: multicore
#[repr(C, packed)]
pub struct PerCPUData {
    stack_ptr: u64,
    thread_rsp: u64,
}

const SYSCALL_STACK_GUARD_PAGE: u64 = 0xFFFF_FF01_0000_0000;
const SYSCALL_STACK_LEN: u64 = 4 * PAGE_SIZE as u64;
const KERNEL_GS_BASE_MSR: u32 = 0xC0000102;

pub static mut PER_CPU_DATA: PerCPUData = PerCPUData {
    stack_ptr: SYSCALL_STACK_GUARD_PAGE + SYSCALL_STACK_LEN,
    thread_rsp: 0,
};

pub fn setup_stack_for_syscall_handler() {
    setup_stack(SYSCALL_STACK_GUARD_PAGE, SYSCALL_STACK_LEN);

    let mut msr = Msr::new(KERNEL_GS_BASE_MSR);
    unsafe {
        msr.write((&raw mut PER_CPU_DATA as *mut PerCPUData) as u64);
    }
}

#[repr(C, packed)]
pub struct SyscallFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub rbp: u64,
    pub rsp: u64,
}

const STAR_MSR: u32 = 0xC0000081;
const LSTAR_MSR: u32 = 0xC0000082;
const FMASK_MSR: u32 = 0xC0000084;

const RING_0: u16 = 0b00;
const RING_3: u16 = 0b11;

// more information from Intel® 64 and IA-32 Architectures
// Software Developer’s Manual
// Volume 3A:
// System Programming Guide, Part 1
// section 5.8.8

pub fn enable_syscalls() {
    let syscall_target_code_segment = KERNEL_CODE_SEGMENT_IDX << 3 | RING_0;
    let sysret_target_code_segment = USER_CODE_SEGMENT_IDX << 3 | RING_3;

    let mut star_msr = Msr::new(STAR_MSR);
    let mut lstar_msr = Msr::new(LSTAR_MSR);
    let mut fmask_msr = Msr::new(FMASK_MSR);
}

#[unsafe(no_mangle)]
extern "C" fn syscall_handler(stack_frame: SyscallFrame) {}

unsafe extern "C" {
    pub unsafe fn syscall_handler_warpper();
}

global_asm!(include_str!("./syscall.s"));
