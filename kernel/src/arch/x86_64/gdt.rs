use crate::arch::x86_64::memory::per_cpu::PerCPUData;
use crate::{get_per_cpu_data, log};
use lazy_static::lazy_static;
use once_cell_no_std::OnceCell;
use x86_64::instructions::segmentation;
use x86_64::instructions::tables::load_tss;
use x86_64::registers::segmentation::{Segment, SegmentSelector};
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable};
use x86_64::structures::tss::TaskStateSegment;

pub const KERNEL_CODE_SEGMENT_IDX: u16 = 1;
pub const USER_CODE_SEGMENT_IDX: u16 = 3;

pub const PAGE_FAULT_IST_INDEX: u16 = 1;

pub const STACK_PAGE_SIZE: usize = 5;
pub const STACK_SIZE: usize = 4096 * STACK_PAGE_SIZE;

pub static TSS: OnceCell<AlignedTSS> = OnceCell::new();

#[derive(Debug)]
#[repr(C, align(16))]
pub struct AlignedTSS(pub TaskStateSegment);

lazy_static! {
    pub static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let kernel_code_selector = gdt.append(Descriptor::kernel_code_segment());
        let kernel_data_selector = gdt.append(Descriptor::kernel_data_segment());
        let user_code_selector = gdt.append(Descriptor::user_code_segment());
        let user_data_selector = gdt.append(Descriptor::user_data_segment());
        let tss_selector = gdt.append(Descriptor::tss_segment(&TSS.get().expect("No TSS found").0));
        (
            gdt,
            Selectors {
                kernel_code_selector,
                kernel_data_selector,
                user_code_selector,
                user_data_selector,
                tss_selector,
            },
        )
    };
}

pub struct Selectors {
    pub kernel_code_selector: SegmentSelector,
    pub kernel_data_selector: SegmentSelector,
    pub user_code_selector: SegmentSelector,
    pub user_data_selector: SegmentSelector,
    pub tss_selector: SegmentSelector,
}

pub fn create_gdt(tss: &'static TaskStateSegment) -> (GlobalDescriptorTable, Selectors) {
    let mut gdt = GlobalDescriptorTable::new();
    let kernel_code_selector = gdt.append(Descriptor::kernel_code_segment());
    let kernel_data_selector = gdt.append(Descriptor::kernel_data_segment());
    let user_code_selector = gdt.append(Descriptor::user_code_segment());
    let user_data_selector = gdt.append(Descriptor::user_data_segment());
    let tss_selector = gdt.append(Descriptor::tss_segment(tss));
    (
        gdt,
        Selectors {
            kernel_code_selector,
            kernel_data_selector,
            user_code_selector,
            user_data_selector,
            tss_selector,
        },
    )
}

pub fn init_gdt() {
    let per_cpu_data = get_per_cpu_data!();
    per_cpu_data.load_gdt();

    log!("GDT initialization finished")
}

impl PerCPUData {
    pub fn load_gdt(&self) {
        let gdt_ref: &'static GlobalDescriptorTable = unsafe { &*(self.gdt.as_ptr()) };
        let selectors: &Selectors = unsafe { &*(self.selectors.as_ptr()) };
        gdt_ref.load();

        unsafe {
            segmentation::CS::set_reg(selectors.kernel_code_selector);
            segmentation::SS::set_reg(selectors.kernel_data_selector);
            load_tss(selectors.tss_selector);
        }
    }
}
