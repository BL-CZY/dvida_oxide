use x86_64::structures::idt::{InterruptStackFrame, PageFaultErrorCode};

use crate::iprintln;

pub extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    iprintln!("[Exception: Break Point]\n{:#?}", stack_frame);
}

pub extern "x86-interrupt" fn pagefault_handler(
    stack_frame: InterruptStackFrame,
    code: PageFaultErrorCode,
) {
    iprintln!("Page fault: {:#?}: {:#?}", stack_frame, code);
}

pub extern "x86-interrupt" fn doublefault_handler(
    stack_frame: InterruptStackFrame,
    err_code: u64,
) -> ! {
    panic!(
        "[Kernal Panic: Double Fault]\nErr Code: {:#?}\n{:#?}",
        err_code, stack_frame
    );
}
