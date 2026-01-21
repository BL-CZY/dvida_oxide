use core::arch::global_asm;

use crate::log;
use x86_64::{
    VirtAddr,
    registers::{
        control::{Efer, EferFlags},
        model_specific::Msr,
        rflags::RFlags,
    },
};

use crate::arch::x86_64::{
    err::ErrNo,
    gdt::GDT,
    memory::{PAGE_SIZE, frame_allocator::setup_stack},
    scheduler::{
        CURRENT_THREAD, PrivilageLevel, State, THREADS, Thread, WAITING_QUEUE, WAITING_QUEUE_IDX,
    },
};

pub const WRITE_SYSCALL: u64 = 1;
pub const KILL_SYSCALL: u64 = 0x3c;

//TODO: multicore
#[repr(C, packed)]
pub struct PerCPUData {
    stack_ptr: u64,
    thread_rsp: u64,
}

const SYSCALL_STACK_GUARD_PAGE: u64 = 0xFFFF_FF81_0000_0000;
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

#[derive(Default)]
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
    pub rsp: u64, // not used for paused tasks for better code structure
}

#[repr(C, packed)]
pub struct LongReturnFrame {
    pub ss: u64,
    pub rsp: u64,
    pub rflags: u64,
    pub cs: u64,
    pub rip: u64,
}

const STAR_MSR: u32 = 0xC0000081;
const LSTAR_MSR: u32 = 0xC0000082;
const FMASK_MSR: u32 = 0xC0000084;

// more information from Intel® 64 and IA-32 Architectures
// Software Developer’s Manual
// Volume 3A:
// System Programming Guide, Part 1
// section 5.8.8

pub fn enable_syscalls() {
    let syscall_target_code_segment = GDT.1.kernel_code_selector.0;
    let sysret_target_code_segment = GDT.1.user_code_selector.0;

    let mask = RFlags::INTERRUPT_FLAG
        | RFlags::DIRECTION_FLAG
        | RFlags::ALIGNMENT_CHECK
        | RFlags::TRAP_FLAG;

    let mut star_msr = Msr::new(STAR_MSR);
    let mut lstar_msr = Msr::new(LSTAR_MSR);
    let mut fmask_msr = Msr::new(FMASK_MSR);

    unsafe {
        star_msr.write(
            (syscall_target_code_segment as u64) << 32 | (sysret_target_code_segment as u64) << 48,
        );
        lstar_msr.write(syscall_handler_wrapper as *const () as u64);
        fmask_msr.write(mask.bits());

        Efer::update(|v| {
            v.set(EferFlags::NO_EXECUTE_ENABLE, true);
            v.set(EferFlags::SYSTEM_CALL_EXTENSIONS, true);
        });
    }
}

#[macro_export]
macro_rules! set_register {
    ($target:ident, $input:ident, $register:ident) => {
        $target.$register = $input.$register
    };
}

#[macro_export]
macro_rules! set_registers {
    ($target:ident, $input:ident) => {
        set_register!($target, $input, rax);
        set_register!($target, $input, rbx);
        set_register!($target, $input, rcx);
        set_register!($target, $input, rdx);
        set_register!($target, $input, rdi);
        set_register!($target, $input, rsi);
        set_register!($target, $input, rbp);
        set_register!($target, $input, r8);
        set_register!($target, $input, r9);
        set_register!($target, $input, r10);
        set_register!($target, $input, r11);
        set_register!($target, $input, r12);
        set_register!($target, $input, r13);
        set_register!($target, $input, r14);
        set_register!($target, $input, r15);
    };
}

#[unsafe(no_mangle)]
extern "C" fn syscall_handler(stack_frame: SyscallFrame) {
    let mut current_thread = CURRENT_THREAD.spin_acquire_lock();
    let mut current_thread = current_thread.take().expect("Corrupted thread context");

    // saves the current thread's registers
    current_thread.state.state = State::Waiting;
    let registers = &mut current_thread.state.registers;

    // save state
    set_registers!(registers, stack_frame);
    current_thread.state.stack_pointer = VirtAddr::new(stack_frame.rsp);

    match stack_frame.rax {
        WRITE_SYSCALL => {
            let idx = WAITING_QUEUE_IDX.fetch_add(1, core::sync::atomic::Ordering::AcqRel);

            // interrupt will be disabled during the handler so this spin will not take too long
            WAITING_QUEUE
                .spin_acquire_lock()
                .insert(idx, current_thread);

            todo!();
        }

        KILL_SYSCALL => {
            log!("Terminating thread: {:?}", current_thread);
        }

        _ => {
            current_thread.state.state = State::Ready;
            registers.rax = ErrNo::OperationNotSupported as u64;

            THREADS.spin_acquire_lock().push_back(current_thread);
        }
    }

    // interrupt will be disabled during the handler so this spin will not take too long
    let thread = THREADS.spin_acquire_lock().pop_front();

    if let Some(mut t) = thread {
        t.ticks_left = 1000;
        resume_thread(t);
    } else {
        panic!("KERNEL THREAD IS DEAD")
    }
}

pub fn resume_thread(thread: Thread) -> ! {
    const IA32_FS_BASE: u32 = 0xC000_0100;

    match thread.state.state {
        State::Paused {
            instruction_pointer,
            rflags,
        } => {
            let mut syscall_frame = SyscallFrame::default();
            let registers = &thread.state.registers;
            set_registers!(syscall_frame, registers);
            syscall_frame.rsp = thread.state.stack_pointer.as_u64();

            let long_return_frame = match thread.privilage_level {
                super::PrivilageLevel::User => LongReturnFrame {
                    ss: GDT.1.user_data_selector.0 as u64,
                    rsp: thread.state.stack_pointer.as_u64(),
                    rflags: rflags.bits(),
                    cs: GDT.1.user_code_selector.0 as u64,
                    rip: instruction_pointer,
                },
                super::PrivilageLevel::Kernel => LongReturnFrame {
                    ss: GDT.1.kernel_data_selector.0 as u64,
                    rsp: thread.state.stack_pointer.as_u64(),
                    rflags: rflags.bits(),
                    cs: GDT.1.kernel_code_selector.0 as u64,
                    rip: instruction_pointer,
                },
            };

            let page_table_pointer = thread.state.page_table_pointer.as_u64();
            const TRUE: u64 = 1;
            const FALSE: u64 = 0;
            let is_kernel = if thread.privilage_level == PrivilageLevel::Kernel {
                TRUE
            } else {
                FALSE
            };

            unsafe {
                Msr::new(IA32_FS_BASE).write(thread.state.thread_local_segment.as_u64());
            }

            *CURRENT_THREAD.spin_acquire_lock() = Some(thread);

            unsafe {
                resume_paused_thread(
                    &syscall_frame as *const SyscallFrame,
                    page_table_pointer,
                    &long_return_frame as *const LongReturnFrame,
                    is_kernel,
                )
            }
        }

        State::Ready => {
            let mut syscall_frame = SyscallFrame::default();
            let registers = &thread.state.registers;
            set_registers!(syscall_frame, registers);
            syscall_frame.rsp = thread.state.stack_pointer.as_u64();

            let page_table_pointer = thread.state.page_table_pointer.as_u64();

            unsafe {
                Msr::new(IA32_FS_BASE).write(thread.state.thread_local_segment.as_u64());
            }

            *CURRENT_THREAD.spin_acquire_lock() = Some(thread);

            unsafe {
                resume_thread_from_syscall(
                    &syscall_frame as *const SyscallFrame,
                    page_table_pointer,
                )
            }
        }

        _ => {
            panic!("Corrupted data");
        }
    }
}

unsafe extern "C" {
    pub unsafe fn syscall_handler_wrapper();
    pub unsafe fn resume_thread_from_syscall(frame: *const SyscallFrame, page_table_ptr: u64) -> !;
    pub unsafe fn resume_paused_thread(
        frame: *const SyscallFrame,
        page_table_pointer: u64,
        long_return_frame: *const LongReturnFrame,
        is_kernel: u64,
    ) -> !;
}

global_asm!(include_str!("./syscall_no_comment.s"));
