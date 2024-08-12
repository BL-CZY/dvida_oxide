use super::gdt;
use super::handlers::{irq, isr};
use lazy_static::lazy_static;
use x86_64::structures::idt::InterruptDescriptorTable;

lazy_static! {
    pub static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(isr::breakpoint_handler);
        unsafe {
            idt.double_fault
                .set_handler_fn(isr::doublefault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt[irq::IrqIndex::Timer as u8].set_handler_fn(irq::timer_handler);
        idt[irq::IrqIndex::Keyboard as u8].set_handler_fn(irq::keyboard_handler);
        idt[irq::IrqIndex::PrimaryIDE as u8].set_handler_fn(irq::primary_ide_handler);
        idt[irq::IrqIndex::SecondaryIDE as u8].set_handler_fn(irq::secondary_ide_handler);
        idt
    };
}

pub fn init_idt() {
    IDT.load();
}
