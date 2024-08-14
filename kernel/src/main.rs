#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![test_runner(run_tests)]
#![reexport_test_harness_main = "test_main"]
use core::arch::asm;
extern crate alloc;

use arch::x86_64::{
    gdt::init_gdt,
    idt::init_idt,
    memory::{self, memmap::log_memmap, PAGE_SIZE},
    pic::init_pic,
};
#[allow(unused_imports)]
use dyn_mem::{allocator::init_kheap, KHEAP_PAGE_COUNT};
use hal::storage::{PRIMARY_STORAGE_CONTEXT, SECONDARY_STORAGE_CONTEXT};
use limine::BaseRevision;

pub mod arch;
pub mod debug;
pub mod drivers;
pub mod dyn_mem;
pub mod hal;
pub mod utils;

// this is the kernel entry point
fn kernel_main() {
    #[cfg(test)]
    test_main();

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
    let (_, kheap_start) = memory::init();
    init_kheap(
        kheap_start as *mut u8,
        (KHEAP_PAGE_COUNT * PAGE_SIZE as u64 - 1) as usize,
    );

    PRIMARY_STORAGE_CONTEXT.lock().init();
    SECONDARY_STORAGE_CONTEXT.lock().init();

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

#[cfg(test)]
pub fn run_tests(tests: &[&dyn Fn()]) {
    use crate::println;
    println!("sadjaskdjaskdaskdjaksljdklasjdlkasjdklasjdkalsjdklasd");
    println!("Running {} tests", tests.len());
    for (index, test) in tests.iter().enumerate() {
        test();
        println!("Test {} succeeded!", index + 1);
    }
}
