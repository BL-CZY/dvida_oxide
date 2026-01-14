use core::arch::naked_asm;

use ejcineque::wakers::{PRIMARY_IDE_WAKERS, SECONDARY_IDE_WAKERS, TIMER_WAKERS};
use x86_64::{
    instructions::port::Port, registers::rflags::RFlags, structures::idt::InterruptStackFrame,
};

use crate::{
    arch::x86_64::{
        handlers::InterruptNoErrcodeFrame,
        pic::{PRIMARY_PIC_OFFSET, get_pic},
        scheduler::{CURRENT_THREAD, THREADS},
    },
    debug::terminal::WRITER,
    hal::keyboard::process_scancode,
    handler_wrapper_noerrcode, set_register, set_registers,
};

#[derive(Debug, Clone, Copy)]
// makes it like c enums
#[repr(u8)]
pub enum IrqIndex {
    Timer = PRIMARY_PIC_OFFSET,
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

        if let Some(ref mut thread) = *CURRENT_THREAD.spin_acquire_lock() {
            thread.ticks_left -= 1;

            if thread.ticks_left == 0 {
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

                THREADS.spin_acquire_lock().push_back(thread);
            }
        }
    });

    unsafe {
        get_pic().notify_end_of_interrupt(IrqIndex::Timer as u8);
    }
}

#[unsafe(naked)]
pub extern "x86-interrupt" fn timer_handler(_stack_frame: InterruptStackFrame) {
    handler_wrapper_noerrcode!(timer_handler_inner);
}

extern "C" fn keyboard_handler_inner(_stack_frame: InterruptNoErrcodeFrame) {
    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    process_scancode(scancode);

    unsafe {
        get_pic().notify_end_of_interrupt(IrqIndex::Keyboard as u8);
    }
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

    unsafe {
        get_pic().notify_end_of_interrupt(IrqIndex::PrimaryIDE as u8);
    }
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

    unsafe {
        get_pic().notify_end_of_interrupt(IrqIndex::SecondaryIDE as u8);
    }
}

#[unsafe(naked)]
pub extern "x86-interrupt" fn secondary_ide_handler(_stack_frame: InterruptStackFrame) {
    handler_wrapper_noerrcode!(secondary_ide_handler_inner);
}
