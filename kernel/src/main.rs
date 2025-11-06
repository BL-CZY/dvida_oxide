#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::debug::test::run_tests)]
#![reexport_test_harness_main = "test_main"]
use core::{arch::asm, ptr::null_mut};
extern crate alloc;

use arch::x86_64::{
    gdt::init_gdt,
    idt::init_idt,
    memory::{self, PAGE_SIZE, memmap::log_memmap},
    pic::init_pic,
};
#[allow(unused_imports)]
use dyn_mem::{KHEAP_PAGE_COUNT, allocator::init_kheap};
use hal::storage::STORAGE_CONTEXT_ARR;
use limine::BaseRevision;

use crate::debug::terminal::DebugWriter;

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
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[unsafe(no_mangle)]
unsafe extern "C" fn _start() -> ! {
    // All limine requests must also be referenced in a called function, otherwise they may be
    // removed by the linker.
    // clear keyboard port
    assert!(BASE_REVISION.is_supported());

    let mut writer: DebugWriter = DebugWriter {
        frame_buffer_width: 0,
        frame_buffer_height: 0,
        frame_buffer_addr: null_mut(),
        terminal_width: 0,
        terminal_height: 0,
        current_row: 0,
        current_col: 0,
        cur_bg_color: 0x000000,
        cur_fg_color: 0xFFFFFF,
        cursor_row: 0,
        cursor_col: 0,
        is_cursor_on: true,
        cursor_blink_interval: 2,
        color_buffer: [[0xFFFFFF00000000; 160]; 100],
        text_buffer: [[b'\0'; 160]; 100],
    };

    writer.init_debug_terminal();
    writer.write_string("we are here\n");

    init_gdt();
    writer.write_string("gdt\n");
    init_idt();
    writer.write_string("idt\n");
    init_pic();
    writer.write_string("pic\n");
    x86_64::instructions::interrupts::enable();
    writer.write_string("interrupts on\n");

    log_memmap();
    writer.write_string("memmap acquired\n");
    let (_, kheap_start) = memory::init();
    init_kheap(
        kheap_start as *mut u8,
        (KHEAP_PAGE_COUNT * PAGE_SIZE as u64 - 1) as usize,
    );
    writer.write_string("kheap!!\n");

    STORAGE_CONTEXT_ARR[hal::storage::PRIMARY].lock().init();
    STORAGE_CONTEXT_ARR[hal::storage::SECONDARY].lock().init();
    writer.write_string("storage?!\n");

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
