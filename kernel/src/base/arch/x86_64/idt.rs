use super::handlers::isr;
use lazy_static::lazy_static;
use x86_64::structures::idt::InterruptDescriptorTable;

lazy_static! {
    pub static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(isr::breakpoint_handler);
        idt
    };
}

pub fn init_idt() {
    IDT.load();
}
