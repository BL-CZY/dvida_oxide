#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![test_runner(debug::test::run_tests)]
use core::arch::asm;

use arch::x86_64::{
    gdt::init_gdt,
    idt::init_idt,
    memory::{self, memmap::log_memmap},
    pic::init_pic,
};
#[allow(unused_imports)]
use debug::test::test_main;
use limine::BaseRevision;

pub mod arch;
pub mod debug;
pub mod drivers;
pub mod dyn_mem;
pub mod hal;
pub mod utils;

// this is the kernel entry point
fn kernel_main() {
    loop {
        x86_64::instructions::hlt();
    }
}

/// Sets the base revision to the latest revision supported by the crate.
/// See specification for further info.
// Be sure to mark all limine requests with #[used], otherwise they may be removed by the compiler.
#[used]
// The .requests section allows limine to find the requests faster and more safely.
#[link_section = ".requests"]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[no_mangle]
unsafe extern "C" fn _start() -> ! {
    // All limine requests must also be referenced in a called function, otherwise they may be
    // removed by the linker.

    // clear keyboard port
    assert!(BASE_REVISION.is_supported());
    debug::terminal::DEFAULT_WRITER.lock().init_debug_terminal();

    init_gdt();
    init_idt();
    init_pic();
    x86_64::instructions::interrupts::enable();

    log_memmap();
    memory::init();

    kernel_main();

    hcf();
}

#[panic_handler]
fn rust_panic(_info: &core::panic::PanicInfo) -> ! {
    println!("{}", _info);
    hcf();
}

fn hcf() -> ! {
    unsafe {
        asm!("cli");
        loop {
            asm!("hlt");
        }
    }
}
