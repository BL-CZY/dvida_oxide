use x86_64::structures::idt::InterruptStackFrame;

use crate::println;

pub extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("[Exception: Break Point]\n{:#?}", stack_frame);
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
