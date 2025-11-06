use pic8259::ChainedPics;

use crate::drivers::keyboard::read_remain_val;

pub const PRIMARY_PIC_OFFSET: u8 = 32;
pub const SECONDARY_PIC_OFFSET: u8 = PRIMARY_PIC_OFFSET + 8;

pub fn get_pic() -> ChainedPics {
    unsafe { ChainedPics::new(PRIMARY_PIC_OFFSET, SECONDARY_PIC_OFFSET) }
}

pub fn init_pic() {
    let mut pics = get_pic();

    unsafe {
        pics.initialize();
        // enable all pics
        pics.write_masks(0, 0);
        read_remain_val();
    }
}
