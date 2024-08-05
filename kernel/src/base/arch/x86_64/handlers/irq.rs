use x86_64::{instructions::port::Port, structures::idt::InterruptStackFrame};

use crate::{
    base::arch::x86_64::pic::{PICS, PRIMARY_PIC_OFFSET},
    print,
};

#[derive(Debug, Clone, Copy)]
// makes it like c enums
#[repr(u8)]
pub enum IrqIndex {
    Timer = PRIMARY_PIC_OFFSET,
    Keyboard = PRIMARY_PIC_OFFSET + 1,
}

pub extern "x86-interrupt" fn timer_handler(_stack_frame: InterruptStackFrame) {
    print!(".");
    unsafe {
        PICS.notify_end_of_interrupt(IrqIndex::Timer as u8);
    }
}

pub extern "x86-interrupt" fn keyboard_handler(_stack_frame: InterruptStackFrame) {
    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    print!("{}", scancode);

    unsafe {
        PICS.notify_end_of_interrupt(IrqIndex::Keyboard as u8);
    }
}
