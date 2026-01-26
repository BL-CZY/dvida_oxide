use core::sync::atomic::AtomicU8;

use crate::arch::x86_64::handlers::irq::IrqIndex;
use crate::arch::x86_64::pic::PRIMARY_ISA_PIC_OFFSET;

use super::gdt;
use super::handlers::{irq, isr};
use crate::log;
use macros::idt_ahci;
use once_cell_no_std::OnceCell;
use x86_64::structures::idt::InterruptDescriptorTable;

// 0-0x20: cpu exceptions
// 0x20-0x30: isa
// 0x30-0x38: ahci
pub const SPURIOUS_INTERRUPT_HANDLER_IDX: u8 = 0xFF;
pub const AHCI_INTERRUPT_HANDLER_IDX: u8 = 0x30;
pub const CUR_AHCI_INTERRUPT_HANDLER_IDX: AtomicU8 = AtomicU8::new(0x30);

static IDT: OnceCell<InterruptDescriptorTable> = OnceCell::new();

pub fn minimal_idt() -> InterruptDescriptorTable {
    let mut idt = InterruptDescriptorTable::new();
    idt.breakpoint.set_handler_fn(isr::breakpoint_handler);
    idt.double_fault.set_handler_fn(isr::doublefault_handler);
    unsafe {
        idt.page_fault
            .set_handler_fn(isr::pagefault_handler)
            .set_stack_index(gdt::PAGE_FAULT_IST_INDEX);
    };

    idt
}

pub fn init_idt(gsi_to_irq_mapping: [u32; 16]) {
    let mut idt = InterruptDescriptorTable::new();
    idt.breakpoint.set_handler_fn(isr::breakpoint_handler);
    idt.double_fault.set_handler_fn(isr::doublefault_handler);

    // the mapping usually maps timer to 2
    idt[PRIMARY_ISA_PIC_OFFSET + gsi_to_irq_mapping[IrqIndex::Timer as usize] as u8]
        .set_handler_fn(irq::timer_handler);
    idt[PRIMARY_ISA_PIC_OFFSET + gsi_to_irq_mapping[IrqIndex::Keyboard as usize] as u8]
        .set_handler_fn(irq::keyboard_handler);
    idt[PRIMARY_ISA_PIC_OFFSET + gsi_to_irq_mapping[IrqIndex::PrimaryIDE as usize] as u8]
        .set_handler_fn(irq::primary_ide_handler);
    idt[PRIMARY_ISA_PIC_OFFSET + gsi_to_irq_mapping[IrqIndex::SecondaryIDE as usize] as u8]
        .set_handler_fn(irq::secondary_ide_handler);
    idt[SPURIOUS_INTERRUPT_HANDLER_IDX].set_handler_fn(isr::spurious_interrupt_handler);
    unsafe {
        idt.page_fault
            .set_handler_fn(isr::pagefault_handler)
            .set_stack_index(gdt::PAGE_FAULT_IST_INDEX);
    };

    idt_ahci!(AHCI_INTERRUPT_HANDLER_IDX);

    let _ = IDT.set(idt);

    IDT.get().expect("Rust error").load();

    log!("IDT initialization finished");
}
