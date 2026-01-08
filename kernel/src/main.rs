#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::debug::test::run_tests)]
#![reexport_test_harness_main = "test_main"]
use core::arch::asm;

use alloc::boxed::Box;
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
        fs::{HalIOCtx, OpenFlags, OpenFlagsValue},
        path::Path,
        storage::{PRIMARY, SECONDARY, run_storage_device},
        vfs::init_vfs,
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

    init_vfs(args.root_drive, args.root_entry).await;
    // let mut inode = hal::vfs::open(
    //     Path::from_str("/lost+found").unwrap(),
    //     // OpenFlags::default(),
    //     OpenFlags {
    //         flags: OpenFlagsValue::CreateIfNotExist as i32,
    //         perms: Some(0),
    //         ..Default::default()
    //     },
    // )
    // .await
    // .unwrap();
    //
    // let mut inode2 = hal::vfs::open(
    //     Path::from_str("/test").unwrap(),
    //     // OpenFlags::default(),
    //     OpenFlags {
    //         flags: OpenFlagsValue::CreateIfNotExist as i32,
    //         perms: Some(0),
    //         ..Default::default()
    //     },
    // )
    // .await
    // .unwrap();

    // log!("created inode: {:?}\n read: {:?}", inode, inode);
    //
    // let mut context = HalIOCtx { head: 0 };
    // // let buf: Box<[u8]> = Box::new([b't', b'e', b's', b't', b's', b't', b'r']);
    // // hal::vfs::write(&mut inode, buf, &mut context)
    // //     .await
    // //     .unwrap();
    // let mut buf: Box<[u8]> = Box::new([0; 7]);
    // hal::vfs::read(&mut inode, &mut buf, &mut context)
    //     .await
    //     .unwrap();
    // log!(
    //     "{:?}",
    //     alloc::string::String::from_utf8_lossy(&buf.to_vec())
    // );
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
