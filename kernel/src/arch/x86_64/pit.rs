use x86_64::instructions::port::{Port, PortGeneric, PortWriteOnly, ReadWriteAccess};

pub const DATA_PORT: u16 = 0x40;
pub const CMD_REGISTER: u16 = 0x43;

pub fn configure_pit() {
    let mut data_port: PortWriteOnly<u8> = PortWriteOnly::new(DATA_PORT);
    let mut cmd_port: Port<u8> = Port::new(DATA_PORT);
    let divisor = 0u16;

    unsafe {
        x86_64::instructions::interrupts::without_interrupts(|| {
            cmd_port.write(0x36);
            data_port.write((divisor & 0xFF) as u8);
            data_port.write(((divisor >> 8) & 0xFF) as u8);
        });
    }
}
