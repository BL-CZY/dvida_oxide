use x86_64::{instructions::port::Port, structures::idt::InterruptStackFrame};

use crate::{
    arch::x86_64::pic::{PICS, PRIMARY_PIC_OFFSET},
    debug::terminal::DEFAULT_WRITER,
    hal::keyboard::process_scancode,
};

#[derive(Debug, Clone, Copy)]
// makes it like c enums
#[repr(u8)]
pub enum IrqIndex {
    Timer = PRIMARY_PIC_OFFSET,
    Keyboard = PRIMARY_PIC_OFFSET + 1,
}

pub extern "x86-interrupt" fn timer_handler(_stack_frame: InterruptStackFrame) {
    unsafe {
        DEFAULT_WRITER.lock().blink_debug_cursor();
        PICS.lock().notify_end_of_interrupt(IrqIndex::Timer as u8);
    }
}

pub extern "x86-interrupt" fn keyboard_handler(_stack_frame: InterruptStackFrame) {
    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    process_scancode(scancode);

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(IrqIndex::Keyboard as u8);
    }
}
