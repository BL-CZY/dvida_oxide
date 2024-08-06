#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::base::debug::test::test_runner)]
use core::arch::asm;

use base::arch::x86_64::{gdt::init_gdt, idt::init_idt, memory::pmm::init_pmm, pic::init_pic};
use limine::BaseRevision;

pub mod base;
pub mod drivers;
pub mod hal;

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
    base::debug::terminal::DEFAULT_WRITER
        .lock()
        .init_debug_terminal();

    init_gdt();
    init_idt();
    init_pic();
    x86_64::instructions::interrupts::enable();

    init_pmm();

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
