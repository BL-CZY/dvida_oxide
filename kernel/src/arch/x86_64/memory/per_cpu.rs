use alloc::collections::btree_map::BTreeMap;
use bytemuck::{Pod, Zeroable};
use limine::mp::Cpu;
use once_cell_no_std::OnceCell;

use crate::arch::x86_64::memory::{
    PAGE_SIZE,
    frame_allocator::{FRAME_ALLOCATOR, setup_stack},
    get_hhdm_offset,
};

pub static PER_CPU_DATA_PTRS: OnceCell<BTreeMap<u32, u64>> = OnceCell::new();

const CURRENT_GS_MSR: u32 = 0xC0000101;

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C, packed)]
pub struct PerCPUData {
    self_ptr: u64,
    syscall_stack_ptr: u64,
    /// used to temporarily save the rsp
    thread_rsp: u64,
    kernel_task_stack_ptr: u64,
    rsp0_stack_ptr: u64,
    page_fault_stack_ptr: u64,
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

const STACK_SIZE: u64 = PAGE_SIZE as u64 * 8;
macro_rules! setup_stack {
    ($per_cpu_data:ident, $field:ident, $cur_stack_base:ident) => {
        $per_cpu_data.$field = setup_stack($cur_stack_base, STACK_SIZE).as_u64();
        $cur_stack_base += STACK_SIZE;
    };
}

pub fn setup_per_cpu_data(cpus: &[&Cpu]) {
    const STACKS_BASE: u64 = 0xFFFF_FF80_0000_0000;

    let mut cur_stack_base = STACKS_BASE;

    let allocator = FRAME_ALLOCATOR.get().expect("Failed to get allocator");

    // allocate per cpu datas
    let per_cpu_data_page_size =
        (size_of::<PerCPUData>() * cpus.len() + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize;

    let frames = allocator
        .spin_acquire_lock()
        .allocate_continuous_frames(&mut None, per_cpu_data_page_size)
        .expect("No memory left");

    let per_cpu_data_start_ptr = get_hhdm_offset() + frames[0].start_address().as_u64();
    let slice = unsafe {
        core::slice::from_raw_parts_mut(
            per_cpu_data_start_ptr.as_mut_ptr() as *mut u8,
            PAGE_SIZE as usize * 2,
        )
    };

    let mut pointers: BTreeMap<u32, u64> = BTreeMap::new();

    for (i, cpu) in cpus.iter().enumerate() {
        let per_cpu_data: &mut PerCPUData = bytemuck::from_bytes_mut(
            &mut slice[i * size_of::<PerCPUData>()
                ..i * size_of::<PerCPUData>() + size_of::<PerCPUData>()],
        );

        setup_stack!(per_cpu_data, syscall_stack_ptr, cur_stack_base);
        setup_stack!(per_cpu_data, page_fault_stack_ptr, cur_stack_base);
        setup_stack!(per_cpu_data, rsp0_stack_ptr, cur_stack_base);

        per_cpu_data.kernel_task_stack_ptr = setup_stack(cur_stack_base, STACK_SIZE * 2).as_u64();
        cur_stack_base += STACK_SIZE * 2;
        per_cpu_data.self_ptr =
            per_cpu_data_start_ptr.as_u64() + size_of::<PerCPUData>() as u64 * i as u64;

        pointers.insert(cpu.id, per_cpu_data.self_ptr);
    }

    let _ = PER_CPU_DATA_PTRS.set(pointers);
}
