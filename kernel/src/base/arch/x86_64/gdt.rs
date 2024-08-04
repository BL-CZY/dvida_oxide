use lazy_static::lazy_static;
use x86_64::instructions::segmentation;
use x86_64::registers::segmentation::{Segment, SegmentSelector};
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable};

lazy_static! {
    static ref GDT: GlobalDescriptorTable = {
        let mut result = GlobalDescriptorTable::new();
        result.append(Descriptor::kernel_code_segment());
        result.append(Descriptor::kernel_data_segment());
        result.append(Descriptor::user_code_segment());
        result.append(Descriptor::user_data_segment());
        result
    };
}

pub fn init_gdt() {
    GDT.load();

    // reload segment registers
    unsafe {
        segmentation::CS::set_reg(SegmentSelector::new(1, x86_64::PrivilegeLevel::Ring0));
        segmentation::SS::set_reg(SegmentSelector::new(2, x86_64::PrivilegeLevel::Ring0));
        segmentation::DS::set_reg(SegmentSelector::new(2, x86_64::PrivilegeLevel::Ring0));
        segmentation::ES::set_reg(SegmentSelector::new(2, x86_64::PrivilegeLevel::Ring0));
        segmentation::FS::set_reg(SegmentSelector::new(2, x86_64::PrivilegeLevel::Ring0));
        segmentation::GS::set_reg(SegmentSelector::new(2, x86_64::PrivilegeLevel::Ring0));
    }
}
