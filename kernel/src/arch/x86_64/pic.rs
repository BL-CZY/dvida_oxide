use crate::log;
use pic8259::ChainedPics;

use crate::drivers::keyboard::read_remain_val;

pub const PRIMARY_ISA_PIC_OFFSET: u8 = 32;
pub const SECONDARY_ISA_PIC_OFFSET: u8 = PRIMARY_ISA_PIC_OFFSET + 8;

pub fn get_pic() -> ChainedPics {
    unsafe { ChainedPics::new(PRIMARY_ISA_PIC_OFFSET, SECONDARY_ISA_PIC_OFFSET) }
}

pub fn init_pic() {
    let mut pics = get_pic();

    unsafe {
        pics.initialize();
        // enable all pics
        pics.write_masks(0, 0);
        read_remain_val();
    }

    log!("PIC initialization finished");
}

pub fn disable_pic() {
    unsafe {
        let mut pics = ChainedPics::new(
            PRIMARY_ISA_PIC_OFFSET + 0x80,
            SECONDARY_ISA_PIC_OFFSET + 0x80,
        );
        pics.initialize();
        pics.write_masks(!0, !0);
    }
}
