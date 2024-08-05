use pic8259::ChainedPics;

use crate::drivers::keyboard::read_remain_val;

pub const PRIMARY_PIC_OFFSET: u8 = 32;
pub const SECONDARY_PIC_OFFSET: u8 = PRIMARY_PIC_OFFSET + 8;

pub static mut PICS: ChainedPics =
    unsafe { ChainedPics::new(PRIMARY_PIC_OFFSET, SECONDARY_PIC_OFFSET) };

pub fn init_pic() {
    unsafe {
        PICS.initialize();
        // enable all pics
        read_remain_val();
        PICS.write_masks(0, 0);
    }
}
