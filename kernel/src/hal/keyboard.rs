use crate::drivers::keyboard::ps2;

pub fn process_scancode(scancode: u8) {
    ps2::read_scancode(scancode);
}
