use core::arch::naked_asm;

use x86_64::structures::idt::{InterruptStackFrame, PageFaultErrorCode};

use crate::{
    arch::x86_64::handlers::{InterruptErrcodeFrame, InterruptNoErrcodeFrame},
    handler_wrapper, iprintln,
};

extern "C" fn breakpoint_handler_inner(stack_frame: InterruptNoErrcodeFrame) {
    iprintln!("[Exception: Break Point]\n{:#?}", stack_frame);
}

#[unsafe(naked)]
pub extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    handler_wrapper!(breakpoint_handler_inner);
}

extern "C" fn pagefault_handler_inner(stack_frame: InterruptErrcodeFrame) {
    let err_code = PageFaultErrorCode::from_bits_truncate(stack_frame.err_code);
    iprintln!("Page fault: {:#?}: {:#?}", stack_frame, err_code);
}

#[unsafe(naked)]
pub extern "x86-interrupt" fn pagefault_handler(
    stack_frame: InterruptStackFrame,
    code: PageFaultErrorCode,
) {
    handler_wrapper!(pagefault_handler_inner);
}

extern "C" fn doublefault_handler_inner(stack_frame: InterruptErrcodeFrame) {
    let err_code = stack_frame.err_code;
    panic!(
        "[Kernal Panic: Double Fault]\nErr Code: {:#?}\n{:#?}",
        err_code, stack_frame
    );
}

#[unsafe(naked)]
pub extern "x86-interrupt" fn doublefault_handler(
    _stack_frame: InterruptStackFrame,
    _err_code: u64,
) -> ! {
    handler_wrapper!(doublefault_handler_inner)
}
