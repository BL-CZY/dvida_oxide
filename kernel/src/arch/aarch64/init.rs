use crate::terminal::WRITER;

pub fn init() -> ! {
    WRITER.lock().init_debug_terminal();

    hcf();
}

pub fn hcf() -> ! {
    loop {
        core::hint::spin_loop();
    }
}
