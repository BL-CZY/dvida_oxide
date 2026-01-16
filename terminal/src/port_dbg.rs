use core::fmt;

use spin::Mutex;
use x86_64::instructions::port::{Port, PortGeneric, ReadWriteAccess};

pub unsafe fn init_serial() {
    let mut data = Port::new(0x3F8);
    let mut int_en = Port::new(0x3F9);
    let mut line_ctrl = Port::new(0x3FB);
    let mut fifo_ctrl = Port::new(0x3FA);
    let mut modem_ctrl = Port::new(0x3FC);

    unsafe {
        int_en.write(0x00 as u8); // Disable interrupts
        line_ctrl.write(0x80 as u8); // Enable DLAB (set baud rate divisor)
        data.write(0x03 as u8); // Set divisor to 3 (38400 baud)
        int_en.write(0x00 as u8); // (High byte of divisor)
        line_ctrl.write(0x03 as u8); // 8 bits, no parity, one stop bit
        fifo_ctrl.write(0xC7 as u8); // Enable FIFO, clear them, with 14-byte threshold
        modem_ctrl.write(0x0B as u8); // IRQs enabled, RTS/DSR Set
    }
}

fn is_transmit_empty() -> bool {
    let mut status_port: PortGeneric<u8, ReadWriteAccess> = Port::new(0x3FD);
    unsafe { (status_port.read() as u8 & 0x20) != 0 }
}

pub struct SerialWriter {}

impl fmt::Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.as_bytes() {
            while !is_transmit_empty() {
                core::hint::spin_loop();
            }
            let mut data_port = Port::new(0x3F8);

            unsafe {
                data_port.write(*c);
            }
        }

        Ok(())
    }
}

pub static SERIAL_WRITER: Mutex<SerialWriter> = Mutex::new(SerialWriter {});

#[doc(hidden)]
#[allow(unused_unsafe, unused)]
pub fn _serial_print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;
    unsafe {
        interrupts::without_interrupts(|| SERIAL_WRITER.lock().write_fmt(args).unwrap());
    }
}

#[macro_export]
macro_rules! serial_iprint {
    ($($arg:tt)*) => ($crate::port_dbg::_serial_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => ($crate::serial_iprint!("{} - line {}, {}\n", file!(), line!(),  format_args!($($arg)*)));
}
