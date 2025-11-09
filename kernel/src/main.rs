#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::debug::test::run_tests)]
#![reexport_test_harness_main = "test_main"]
use core::arch::asm;

use alloc::boxed::Box;
use terminal::iprintln;

extern crate alloc;

use arch::x86_64::{
    gdt::init_gdt,
    idt::init_idt,
    memory::{self, PAGE_SIZE, memmap::log_memmap},
    pic::init_pic,
};
#[allow(unused_imports)]
use dyn_mem::{KHEAP_PAGE_COUNT, allocator::init_kheap};
use ejcineque::{
    executor::Executor,
    sync::mpsc::unbounded::{UnboundedReceiver, unbounded_channel},
};
use hal::storage::STORAGE_CONTEXT_ARR;
use limine::{BaseRevision, request::StackSizeRequest};

use crate::{
    arch::x86_64::{memory::MemoryMappings, pit::configure_pit},
    debug::terminal::WRITER,
    hal::storage::{
        HalStorageOperation, HalStorageOperationResult, PRIMARY, PRIMARY_STORAGE_SENDER, SECONDARY,
        SECONDARY_STORAGE_SENDER, run_storage_device,
    },
};

pub mod arch;
pub mod debug;
pub mod drivers;
pub mod dyn_mem;
pub mod hal;
pub mod utils;

pub const STACK_SIZE: u64 = 0x100000;
pub static STACK_SIZE_REQUEST: StackSizeRequest = StackSizeRequest::new().with_size(STACK_SIZE);

// this is the kernel entry point
async fn kernel_main(executor: Executor) {
    #[cfg(test)]
    test_main();

    let (primary_storage_tx, primary_storage_rx) = unbounded_channel::<HalStorageOperation>();
    let _ = PRIMARY_STORAGE_SENDER
        .set(primary_storage_tx)
        .expect("Failed to put the primary storage sender");
    iprintln!("{:?}", PRIMARY_STORAGE_SENDER);
    executor.spawn(run_storage_device(PRIMARY, primary_storage_rx));

    let (secondary_storage_tx, secondary_storage_rx) = unbounded_channel::<HalStorageOperation>();
    let _ = SECONDARY_STORAGE_SENDER
        .set(secondary_storage_tx)
        .expect("Failed to put the secondary storage sender");
    executor.spawn(run_storage_device(SECONDARY, secondary_storage_rx));

    let sender = PRIMARY_STORAGE_SENDER.get().unwrap().clone();
    let buffer = [0u8; 512];
    let (tx, rx) = unbounded_channel::<HalStorageOperationResult>();
    sender.send(HalStorageOperation::Read {
        buffer: Box::new(buffer),
        lba: 0,
        sender: tx,
    });

    if let Some(a) = rx.recv().await {
        iprintln!("read stuff: {:?}", a);
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

    WRITER.lock().init_debug_terminal();

    init_gdt();
    init_idt();
    init_pic();
    x86_64::instructions::interrupts::enable();
    configure_pit();

    log_memmap();
    let MemoryMappings { kheap, .. } = memory::init();
    init_kheap(
        kheap.kheap_start,
        (KHEAP_PAGE_COUNT * PAGE_SIZE as u64 - 1) as usize,
    );

    STORAGE_CONTEXT_ARR[hal::storage::PRIMARY].lock().init();
    STORAGE_CONTEXT_ARR[hal::storage::SECONDARY].lock().init();

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
