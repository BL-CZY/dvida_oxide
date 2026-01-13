use x86_64::{VirtAddr, registers::model_specific::Msr};

use crate::arch::x86_64::memory::{PAGE_SIZE, frame_allocator::setup_stack};

//TODO: multicore
#[repr(C)]
pub struct PerCPUData {
    stack_ptr: VirtAddr,
}

const SYSCALL_STACK_GUARD_PAGE: u64 = 0xFFFF_FF01_0000_0000;
const SYSCALL_STACK_LEN: u64 = 4 * PAGE_SIZE as u64;
const KERNEL_GS_BASE: u32 = 0xC0000102;

pub const PER_CPU_DATA: PerCPUData = PerCPUData {
    stack_ptr: VirtAddr::new(SYSCALL_STACK_GUARD_PAGE + SYSCALL_STACK_LEN),
};

pub fn setup_stack_for_syscall_handler() {
    setup_stack(SYSCALL_STACK_GUARD_PAGE, SYSCALL_STACK_LEN);

    let mut msr = Msr::new(KERNEL_GS_BASE);
    unsafe {
        msr.write((&PER_CPU_DATA as *const PerCPUData) as u64);
    }
}
