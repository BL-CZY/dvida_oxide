#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::debug::test::run_tests)]
#![reexport_test_harness_main = "test_main"]
use core::arch::asm;

use terminal::{iprintln, log};

extern crate alloc;

use arch::x86_64::{
    gdt::init_gdt,
    idt::init_idt,
    memory::{self, PAGE_SIZE, memmap::log_memmap},
    pic::init_pic,
};
#[allow(unused_imports)]
use dyn_mem::{KHEAP_PAGE_COUNT, allocator::init_kheap};
use ejcineque::{executor::Executor, futures::yield_now};
use hal::storage::STORAGE_CONTEXT_ARR;
use limine::{BaseRevision, request::StackSizeRequest};
pub mod args;
pub mod time;

use crate::{
    arch::x86_64::{memory::MemoryMappings, pit::configure_pit},
    args::parse_args,
    crypto::random::run_random,
    debug::terminal::WRITER,
    hal::{
        storage::{PRIMARY, SECONDARY, run_storage_device},
        vfs::{init_vfs, spawn_vfs_task},
    },
};

pub mod arch;
pub mod crypto;
pub mod debug;
pub mod drivers;
pub mod dyn_mem;
pub mod hal;

pub const STACK_SIZE: u64 = 0x100000;
pub static STACK_SIZE_REQUEST: StackSizeRequest = StackSizeRequest::new().with_size(STACK_SIZE);

// this is the kernel entry point
async fn kernel_main(executor: Executor) {
    #[cfg(test)]
    test_main();

    log!("Kernel main launched");

    executor.spawn(run_storage_device(PRIMARY));
    // we yield now to let the tasks actually initialize
    yield_now().await;

    executor.spawn(run_storage_device(SECONDARY));
    yield_now().await;

    log!("Storage drive tasks launched");

    executor.spawn(run_random());
    yield_now().await;
    log!("Random number task launched");

    let args = parse_args();

    executor.spawn(spawn_vfs_task(args.root_drive, args.root_entry));
    yield_now().await;
    log!("VFS task launched")
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

    WRITER.lock().init_debug_terminal();

    init_gdt();
    init_idt();
    init_pic();
    x86_64::instructions::interrupts::enable();
    log!("Interrupts enabled!");
    configure_pit();

    log_memmap();
    let MemoryMappings { kheap, .. } = memory::init();
    init_kheap(
        kheap.kheap_start,
        (KHEAP_PAGE_COUNT * PAGE_SIZE as u64 - 1) as usize,
    );

    STORAGE_CONTEXT_ARR[hal::storage::PRIMARY].lock().init();
    STORAGE_CONTEXT_ARR[hal::storage::SECONDARY].lock().init();
    log!("Initialized the storage drives");

    let executor: Executor = Executor::new();
    executor.spawn(kernel_main(executor.clone()));

    executor.run();

    hcf();
}

#[panic_handler]
fn rust_panic(_info: &core::panic::PanicInfo) -> ! {
    iprintln!("{}", _info);
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
