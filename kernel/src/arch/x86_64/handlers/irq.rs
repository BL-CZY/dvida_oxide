use core::{arch::naked_asm, time::Duration};

use crate::{
    arch::x86_64::timer::MILLISECOND_TO_NANO_SECOND,
    drivers::ata::sata::task::ahci_interrupt_handler_by_idx,
    ejcineque::wakers::{PRIMARY_IDE_WAKERS, SECONDARY_IDE_WAKERS, TIMER_WAKERS},
    get_per_cpu_data_mut,
};
use macros::ahci_interrupt_handler_template;
use x86_64::{
    VirtAddr, instructions::port::Port, registers::rflags::RFlags,
    structures::idt::InterruptStackFrame,
};

use crate::{
    arch::x86_64::{
        acpi::apic::get_local_apic,
        handlers::InterruptNoErrcodeFrame,
        scheduler::{DEFAULT_TICKS_PER_THREAD, syscall::resume_thread},
    },
    hal::keyboard::process_scancode,
    handler_wrapper_noerrcode, set_register, set_registers,
    terminal::WRITER,
};

#[derive(Debug, Clone, Copy)]
#[repr(usize)]
pub enum IrqIndex {
    Timer = 0,
    Keyboard,
    Cascade,
    Com24,
    Com13,
    Sound,
    Floppy,
    Printer,
    Clock,
    Video,
    Open1,
    Open2,
    Mouse,
    Coprocessor,
    PrimaryIDE,
    SecondaryIDE,
}

extern "C" fn timer_handler_inner(stack_frame: InterruptNoErrcodeFrame) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        for w in TIMER_WAKERS.lock().drain(..) {
            w.wake();
        }
        WRITER.lock().blink_debug_cursor();

        let per_cpu_data = get_per_cpu_data_mut!();

        if let Some(current_thread_idx) = per_cpu_data.scheduler_context.current_thread {
            if let Some(ref mut thread) = per_cpu_data
                .scheduler_context
                .thread_map
                .get_mut(&current_thread_idx)
            {
                thread.time_left.saturating_sub(Duration::from_nanos_u128(
                    MILLISECOND_TO_NANO_SECOND / per_cpu_data.apic_timer_ticks_per_ms as u128,
                ));

                if thread.time_left.is_zero() {
                    let registers = &mut thread.state.registers;

                    set_registers!(registers, stack_frame);
                    thread.state.state = crate::arch::x86_64::scheduler::State::Paused {
                        instruction_pointer: stack_frame.rip,
                        rflags: RFlags::from_bits_retain(stack_frame.rflags),
                    };
                    thread.state.stack_pointer = VirtAddr::new(stack_frame.rsp);

                    let threads = &mut per_cpu_data.scheduler_context.thread_queue;
                    threads.push_back(current_thread_idx);

                    while let Some(thread_id) =
                        per_cpu_data.scheduler_context.thread_queue.pop_front()
                    {
                        if let Some(thread) = per_cpu_data
                            .scheduler_context
                            .thread_map
                            .get_mut(&thread_id)
                        {
                            thread.time_left = DEFAULT_TICKS_PER_THREAD;
                            resume_thread(thread);
                        }
                    }
                    panic!("KERNEL THREAD IS DEAD")
                }
            }
        }
    });

    get_local_apic().write_eoi(0);
}

#[unsafe(naked)]
pub extern "x86-interrupt" fn timer_handler(_stack_frame: InterruptStackFrame) {
    handler_wrapper_noerrcode!(timer_handler_inner);
}

extern "C" fn keyboard_handler_inner(_stack_frame: InterruptNoErrcodeFrame) {
    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    process_scancode(scancode);

    get_local_apic().write_eoi(0);
}

#[unsafe(naked)]
pub extern "x86-interrupt" fn keyboard_handler(_stack_frame: InterruptStackFrame) {
    handler_wrapper_noerrcode!(keyboard_handler_inner);
}

extern "C" fn primary_ide_handler_inner(_stack_frame: InterruptNoErrcodeFrame) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        for w in PRIMARY_IDE_WAKERS.lock().drain(..) {
            w.wake();
        }
    });

    get_local_apic().write_eoi(0);
}

#[unsafe(naked)]
pub extern "x86-interrupt" fn primary_ide_handler(_stack_frame: InterruptStackFrame) {
    handler_wrapper_noerrcode!(primary_ide_handler_inner);
}

extern "C" fn secondary_ide_handler_inner(_stack_frame: InterruptNoErrcodeFrame) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        for w in SECONDARY_IDE_WAKERS.lock().drain(..) {
            w.wake();
        }
    });

    get_local_apic().write_eoi(0);
}

#[unsafe(naked)]
pub extern "x86-interrupt" fn secondary_ide_handler(_stack_frame: InterruptStackFrame) {
    handler_wrapper_noerrcode!(secondary_ide_handler_inner);
}

ahci_interrupt_handler_template!();
