use x86_64::instructions::port::Port;

pub mod ps2;

pub fn read_remain_val() {
    let mut port = Port::new(0x60);
    let _scancode: u8 = unsafe { port.read() };
}
