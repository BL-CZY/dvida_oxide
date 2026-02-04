use core::mem::MaybeUninit;

use alloc::collections::btree_map::BTreeMap;
use limine::mp::Cpu;
use once_cell_no_std::OnceCell;
use x86_64::{
    VirtAddr,
    structures::{gdt::GlobalDescriptorTable, tss::TaskStateSegment},
};

use crate::arch::x86_64::{
    gdt::{self, Selectors, create_gdt},
    memory::{
        PAGE_SIZE,
        frame_allocator::{FRAME_ALLOCATOR, setup_stack},
        get_hhdm_offset,
    },
};

pub static PER_CPU_DATA_PTRS: OnceCell<BTreeMap<u32, u64>> = OnceCell::new();

pub const CURRENT_GS_MSR: u32 = 0xC0000101;

#[repr(C, align(128))]
pub struct PerCPUData {
    pub self_ptr: u64,
    pub syscall_stack_ptr: u64,
    /// used to temporarily save the rsp
    pub thread_rsp: u64,
    pub kernel_task_stack_ptr: u64,
    pub rsp0_stack_ptr: u64,
    pub page_fault_stack_ptr: u64,
    /// the upper 32 bits can be used
    pub id: u64,
    pub gdt: MaybeUninit<GlobalDescriptorTable>,
    pub tss: TaskStateSegment,
    pub selectors: MaybeUninit<Selectors>,

    pub tsc_offset: i64,
}

#[macro_export]
macro_rules! get_per_cpu_data {
    () => {
        unsafe {
            &*(x86_64::registers::model_specific::Msr::new(
                $crate::arch::x86_64::memory::per_cpu::CURRENT_GS_MSR,
            )
            .read() as *mut $crate::arch::x86_64::memory::per_cpu::PerCPUData)
        }
    };
}

#[macro_export]
macro_rules! get_per_cpu_data_mut {
    () => {
        unsafe {
            &mut *(x86_64::registers::model_specific::Msr::new(
                $crate::arch::x86_64::memory::per_cpu::CURRENT_GS_MSR,
            )
            .read() as *mut $crate::arch::x86_64::memory::per_cpu::PerCPUData)
        }
    };
}

const STACK_SIZE: u64 = PAGE_SIZE as u64 * 8;
macro_rules! setup_stack {
    ($cur_stack_base:ident, $head:ident) => {
        let $head = setup_stack($cur_stack_base, STACK_SIZE).as_u64();
        $cur_stack_base += STACK_SIZE;
    };
}

pub fn setup_per_cpu_data(cpus: &[&Cpu]) {
    assert!(core::mem::offset_of!(PerCPUData, self_ptr) == 0x00);
    assert!(core::mem::offset_of!(PerCPUData, syscall_stack_ptr) == 0x08);
    assert!(core::mem::offset_of!(PerCPUData, thread_rsp) == 0x10);

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

    let mut pointers: BTreeMap<u32, u64> = BTreeMap::new();

    for (i, cpu) in cpus.iter().enumerate() {
        let ptr = per_cpu_data_start_ptr.as_u64() + size_of::<PerCPUData>() as u64 * i as u64;

        setup_stack!(cur_stack_base, syscall_stack_ptr);
        setup_stack!(cur_stack_base, rsp0_stack_ptr);
        setup_stack!(cur_stack_base, page_fault_stack_ptr);

        let kernel_task_stack_ptr = setup_stack(cur_stack_base, STACK_SIZE * 2).as_u64();
        cur_stack_base += STACK_SIZE * 2;

        pointers.insert(cpu.id, ptr);

        let tss = {
            let mut tss = TaskStateSegment::new();
            tss.interrupt_stack_table[gdt::PAGE_FAULT_IST_INDEX as usize] =
                VirtAddr::from_ptr(page_fault_stack_ptr as *mut u8);
            tss.privilege_stack_table[0] = VirtAddr::new(rsp0_stack_ptr);
            tss
        };

        unsafe {
            (ptr as *mut PerCPUData).write(PerCPUData {
                self_ptr: ptr,
                syscall_stack_ptr,
                thread_rsp: 0,
                kernel_task_stack_ptr,
                rsp0_stack_ptr,
                page_fault_stack_ptr,
                id: cpu.id as u64,
                gdt: MaybeUninit::uninit(),
                selectors: MaybeUninit::uninit(),
                tss,
                tsc_offset: 0,
            });
        }

        let per_cpu_data = unsafe { &mut *(ptr as *mut PerCPUData) };
        let tss_ref: &'static TaskStateSegment = &per_cpu_data.tss;
        let (gdt, selectors) = create_gdt(tss_ref);
        per_cpu_data.gdt.write(gdt);
        per_cpu_data.selectors.write(selectors);
    }

    let _ = PER_CPU_DATA_PTRS.set(pointers);
}
