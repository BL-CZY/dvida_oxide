#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
use core::arch::asm;

use base::arch::x86_64::{gdt::init_gdt, idt::init_idt};
use limine::BaseRevision;

pub mod base;

// this is the kernel entry point
fn kernel_main() {
    println!("Hello World!");
    x86_64::instructions::interrupts::int3();
    loop {
        unsafe {
            asm!("hlt");
        }
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
    assert!(BASE_REVISION.is_supported());
    base::debug::terminal::DEFAULT_WRITER.init_debug_terminal();

    init_gdt();
    init_idt();

    kernel_main();

    hcf();
}

#[cfg(not(test))]
#[panic_handler]
fn rust_panic(_info: &core::panic::PanicInfo) -> ! {
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
