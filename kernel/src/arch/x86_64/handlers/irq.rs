use core::arch::naked_asm;

use crate::{
    drivers::ata::sata::task::ahci_interrupt_handler_by_idx,
    ejcineque::wakers::{PRIMARY_IDE_WAKERS, SECONDARY_IDE_WAKERS, TIMER_WAKERS},
    log,
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
        scheduler::{CURRENT_THREAD, DEFAULT_TICKS_PER_THREAD, THREADS, syscall::resume_thread},
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

        let mut current_thread_guard = CURRENT_THREAD.spin_acquire_lock();

        let mut ticks_left = 1;
        if let Some(ref mut thread) = *current_thread_guard {
            thread.ticks_left -= 1;
            ticks_left = thread.ticks_left;
        }

        drop(current_thread_guard);

        if ticks_left == 0 {
            // takes it
            let mut thread = CURRENT_THREAD
                .spin_acquire_lock()
                .take()
                .expect("Impossible error");

            let registers = &mut thread.state.registers;

            set_registers!(registers, stack_frame);
            thread.state.state = crate::arch::x86_64::scheduler::State::Paused {
                instruction_pointer: stack_frame.rip,
                rflags: RFlags::from_bits_retain(stack_frame.rflags),
            };
            thread.state.stack_pointer = VirtAddr::new(stack_frame.rsp);

            let mut threads = THREADS.spin_acquire_lock();
            threads.push_back(thread);
            let thread = threads.pop_front();

            drop(threads);

            if let Some(mut t) = thread {
                t.ticks_left = DEFAULT_TICKS_PER_THREAD;
                // unsafe {
                //     get_pic().notify_end_of_interrupt(IrqIndex::Timer as u8);
                // }

                resume_thread(t);
            } else {
                panic!("KERNEL THREAD IS DEAD")
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
