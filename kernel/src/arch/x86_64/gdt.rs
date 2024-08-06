use core::ptr::addr_of;

use lazy_static::lazy_static;
use x86_64::instructions::segmentation;
use x86_64::instructions::tables::load_tss;
use x86_64::registers::segmentation::{Segment, SegmentSelector};
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            #[allow(unused_unsafe)]
            let stack_start = VirtAddr::from_ptr(unsafe { addr_of!(STACK) });
            let stack_end = stack_start + STACK_SIZE.try_into().unwrap();
            stack_end
        };
        tss
    };
}

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let kernel_code_selector = gdt.append(Descriptor::kernel_code_segment());
        let kernel_data_selector = gdt.append(Descriptor::kernel_data_segment());
        gdt.append(Descriptor::user_code_segment());
        gdt.append(Descriptor::user_data_segment());
        let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));
        (
            gdt,
            Selectors {
                kernel_code_selector,
                kernel_data_selector,
                tss_selector,
            },
        )
    };
}

struct Selectors {
    kernel_code_selector: SegmentSelector,
    kernel_data_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

pub fn init_gdt() {
    GDT.0.load();

    // reload segment registers
    unsafe {
        segmentation::CS::set_reg(GDT.1.kernel_code_selector);
        segmentation::SS::set_reg(GDT.1.kernel_data_selector);
        segmentation::DS::set_reg(GDT.1.kernel_data_selector);
        segmentation::ES::set_reg(GDT.1.kernel_data_selector);
        segmentation::FS::set_reg(GDT.1.kernel_data_selector);
        segmentation::GS::set_reg(GDT.1.kernel_data_selector);
        load_tss(GDT.1.tss_selector);
    }
}
