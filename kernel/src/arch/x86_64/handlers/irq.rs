use x86_64::{instructions::port::Port, structures::idt::InterruptStackFrame};

use crate::{
    arch::x86_64::pic::{PRIMARY_PIC_OFFSET, get_pic},
    debug::terminal::WRITER,
    hal::keyboard::process_scancode,
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

pub extern "x86-interrupt" fn timer_handler(_stack_frame: InterruptStackFrame) {
    unsafe {
        WRITER.lock().blink_debug_cursor();
        get_pic().notify_end_of_interrupt(IrqIndex::Timer as u8);
    }
}

pub extern "x86-interrupt" fn keyboard_handler(_stack_frame: InterruptStackFrame) {
    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    process_scancode(scancode);

    unsafe {
        get_pic().notify_end_of_interrupt(IrqIndex::Keyboard as u8);
    }
}

pub extern "x86-interrupt" fn primary_ide_handler(_stack_frame: InterruptStackFrame) {
    //TODO stop polling ig
    unsafe {
        get_pic().notify_end_of_interrupt(IrqIndex::PrimaryIDE as u8);
    }
}

pub extern "x86-interrupt" fn secondary_ide_handler(_stack_frame: InterruptStackFrame) {
    unsafe {
        get_pic().notify_end_of_interrupt(IrqIndex::SecondaryIDE as u8);
    }
}
