use alloc::collections::BTreeMap;
use lazy_static::lazy_static;
use x86_64::VirtAddr;

use crate::ejcineque::sync::mutex::Mutex;

const CURRENT_GS_MSR: u32 = 0xC0000101;

pub const STACK_COUNT: usize = 10;

pub enum StackIdx {}

lazy_static! {
    pub static ref STACKS: Mutex<BTreeMap<u32, [VirtAddr; STACK_COUNT]>> =
        Mutex::new(BTreeMap::new());
}

//TODO: multicore
#[repr(C, packed)]
pub struct PerCPUData {
    self_ptr: u64,
    /// used to track the kernel stack pointer
    stack_ptr: u64,
    /// used to temporarily save the rsp
    thread_rsp: u64,
}

#[macro_export]
macro_rules! get_per_cpu_data {
    () => {
        let res = unsafe {
            &mut *(x86_64::registers::model_specific::Msr::new(CURRENT_GS_MSR).read()
                as *mut PerCPUData)
        };

        res
    };
}
